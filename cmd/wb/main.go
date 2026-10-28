package main

import (
	"context"
	"flag"
	"fmt"
	"io"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	"github.com/aaradhychinche-alt/WhatBreaks/internal/answer"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/api"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/collector/k8s"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/config"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/coreclient"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/database"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/discovery"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/health"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/logging"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/platform"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/scheduler"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/state"
)

var (
	version = "0.1.0-dev"
)

func main() {
	code := run(os.Args, os.Getenv, os.Stdout, os.Stderr, nil)
	os.Exit(code)
}

func run(args []string, getenv func(string) string, stdout, stderr io.Writer, testCtx context.Context) int {
	fs := flag.NewFlagSet(args[0], flag.ContinueOnError)
	fs.SetOutput(stderr)

	showHelp := fs.Bool("help", false, "Show help message")
	showVersion := fs.Bool("version", false, "Show version")
	apiPort := fs.Int("api-port", 8080, "HTTP API server port")
	healthPort := fs.Int("health-port", 8081, "HTTP health probe server port")
	coreAddr := fs.String("core-engine-addr", config.DefaultCoreEngineAddress, "Rust Core Engine gRPC endpoint")
	workspaceID := fs.String("workspace-id", "00000000-0000-0000-0000-000000000001", "Workspace UUID identifier")
	k8sEnable := fs.Bool("enable-k8s", true, "Enable Kubernetes collector")
	k8sAPIURL := fs.String("k8s-api-url", "", "Kubernetes API server URL")
	k8sClusterID := fs.String("k8s-cluster-id", "k8s-cluster", "Kubernetes cluster identifier")
	k8sClusterWide := fs.Bool("k8s-cluster-wide", false, "Enable cluster-wide Kubernetes collection")
	k8sNamespaces := fs.String("k8s-namespaces", "", "Comma-separated list of namespaces to observe")

	if len(args) > 1 {
		switch args[1] {
		case "help":
			printUsage(fs, stdout)
			return 0
		case "version":
			fmt.Fprintf(stdout, "WhatBreaks Platform %s\n", version)
			return 0
		}
	}

	if err := fs.Parse(args[1:]); err != nil {
		return 2
	}

	if *showHelp {
		printUsage(fs, stdout)
		return 0
	}

	if *showVersion {
		fmt.Fprintf(stdout, "WhatBreaks Platform %s\n", version)
		return 0
	}

	// The standard flag package stops parsing at the first positional argument.
	// Accept help/version after flags as well as before them.
	if positional := fs.Args(); len(positional) > 0 {
		switch positional[0] {
		case "help":
			printUsage(fs, stdout)
			return 0
		case "version":
			fmt.Fprintf(stdout, "WhatBreaks Platform %s\n", version)
			return 0
		}
	}

	logger := logging.NewStandardLogger(stderr, logging.LevelInfo)
	if *k8sClusterID == "k8s-cluster" || *k8sClusterID == "" {
		if detected := detectKubeContext(); detected != "" {
			*k8sClusterID = detected
		}
	}
	logger.Info("Initializing WhatBreaks Platform", "version", version, "cluster_id", *k8sClusterID)

	if getenv == nil {
		getenv = os.Getenv
	}

	// Verify forbidden environment variable invariants (anti-credential leakage)
	if getenv(config.EnvWbApiToken) != "" || getenv(config.EnvTtApiToken) != "" {
		logger.Error(config.ErrForbiddenRawTokenEnv.Error())
		return 1
	}
	if getenv(config.EnvKubeconfig) != "" {
		logger.Error(config.ErrForbiddenKubeconfigEnv.Error())
		return 1
	}

	envLookup := func(k string) (string, bool) {
		v := getenv(k)
		return v, v != ""
	}

	cfg := &config.PlatformConfig{
		Health: config.HealthConfig{
			Port: *healthPort,
		},
		CoreEngine: config.CoreEngineConfig{
			Address: *coreAddr,
			Timeout: config.DefaultCoreEngineTimeout,
		},
		Logging: config.LoggingConfig{
			Level:       config.DefaultLogLevel,
			ServiceName: "whatbreaks-platform",
		},
	}

	// 1. Health State and Probe Server
	healthState := health.NewState()
	healthSrv := health.NewServer("0.0.0.0", *healthPort, healthState, logger)

	// 2. HTTP API Server Config
	apiCfg := api.NewConfigFromEnv(envLookup, logger)
	if *apiPort != api.DefaultPort {
		apiCfg.Port = *apiPort
	}
	apiCfg.ConfiguredWorkspaceID = *workspaceID

	// 3. Worker Task Scheduler
	sched := scheduler.New(scheduler.WithLogger(logger))

	// 4. Assembled Platform
	var platformOpts []platform.Option
	platformOpts = append(platformOpts,
		platform.WithHealthServer(healthSrv),
		platform.WithHealthState(healthState),
		platform.WithScheduler(sched),
	)

	// Connect Database if configured
	var pgStore state.Store
	dbCtx, dbCancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer dbCancel()

	if dbCfg, err := config.LoadDatabaseConfig(envLookup); err == nil {
		db, dbErr := database.New(dbCtx, dbCfg, database.WithLogger(logger))
		if dbErr == nil {
			if (apiCfg.IsDevelopment || apiCfg.IsTest) && dbCfg.User == config.DefaultDbUser {
				if testErr := db.TestConnection(dbCtx); testErr != nil {
					// In development or test, retry with local development user "postgres" if default "whatbreaks" role is not configured
					fallbackCfg := *dbCfg
					fallbackCfg.User = "postgres"
					fallbackCfg.Database = "postgres"
					if altDb, altErr := database.New(dbCtx, &fallbackCfg, database.WithLogger(logger)); altErr == nil {
						if altDb.TestConnection(dbCtx) == nil {
							db = altDb
							dbCfg = &fallbackCfg
							dbErr = nil
						}
					}
				}
			}
		}

		if dbErr == nil && db.TestConnection(dbCtx) == nil {
			if schemaErr := state.EnsureSchema(dbCtx, db); schemaErr != nil {
				logger.Error("Failed to apply database schema", "error", schemaErr)
			}
			pgStore = state.NewPostgresStore(db)
			apiCfg.Store = pgStore
			apiCfg.Database = db
			apiCfg.HealthPinger = db
			apiCfg.DatabaseInfo = map[string]any{
				"host":     dbCfg.Host,
				"port":     dbCfg.Port,
				"database": dbCfg.Database,
			}
			platformOpts = append(platformOpts, platform.WithDatabase(db))
			logger.Info("Connected to PostgreSQL state database", "host", dbCfg.Host, "db", dbCfg.Database, "user", dbCfg.User)
		} else {
			logger.Warn("PostgreSQL not connected or auth failed, running in stateless mode", "host", dbCfg.Host, "db", dbCfg.Database)
		}
	}

	// Connect Rust Core Engine gRPC Client
	var coreClient *coreclient.Client
	if *coreAddr != "" {
		grpcCtx, grpcCancel := context.WithTimeout(context.Background(), 2*time.Second)
		defer grpcCancel()
		if client, err := coreclient.Connect(grpcCtx, *coreAddr); err == nil {
			coreClient = client
			var answerOpts []answer.Option
			if pgStore != nil {
				answerOpts = append(answerOpts,
					answer.WithWorkspaceIntegrityChecker(pgStore),
					answer.WithRequireIntegrityChecker(),
				)
			}
			answerSvc := answer.NewService(coreClient, logger, answerOpts...)
			apiCfg.ImpactHandler = answer.NewHandler(
				answerSvc,
				answer.WithLogger(logger),
				answer.WithAuthorizer(apiCfg.WorkspaceAuthorizer, nil),
			)
			apiCfg.CoreChecker = func(ctx context.Context) error {
				_, err := client.CheckLiveness(ctx)
				return err
			}
			logger.Info("Connected to Rust Core Engine", "address", *coreAddr)

			if pgStore != nil {
				materializer := state.NewMaterializer(pgStore, coreClient, logger)
				reconciler := state.NewReconciler(pgStore, coreClient, materializer, logger)
				apiCfg.Reconciler = reconciler

				recoveryMgr := state.NewRecoveryManager(
					pgStore,
					materializer,
					coreClient,
					healthState,
					logger,
				)
				platformOpts = append(platformOpts, platform.WithRecoveryManager(recoveryMgr))
			}
		} else {
			logger.Info("Rust Core Engine not reachable, impact analysis disabled", "address", *coreAddr, "reason", err.Error())
		}
	}

	// 5. Initialize Kubernetes Collector if enabled
	if *k8sEnable || *k8sAPIURL != "" {
		k8sClient, err := k8s.NewRESTClient(k8s.ClientConfig{
			BaseURL: *k8sAPIURL,
			Logger:  logger,
		})
		if err != nil {
			logger.Error("Failed to initialize Kubernetes client", "error", err)
			return 1
		}
		var namespaces []string
		if *k8sNamespaces != "" {
			for _, ns := range strings.Split(*k8sNamespaces, ",") {
				if t := strings.TrimSpace(ns); t != "" {
					namespaces = append(namespaces, t)
				}
			}
		}
		colOpts := []k8s.Option{
			k8s.WithClient(k8sClient),
			k8s.WithClusterID(*k8sClusterID),
			k8s.WithWorkspaceID(*workspaceID),
			k8s.WithClusterWide(*k8sClusterWide),
			k8s.WithNamespaces(namespaces...),
			k8s.WithLogger(logger),
		}
		if coreClient != nil {
			colOpts = append(colOpts, k8s.WithCoreClient(coreClient))
		}
		k8sCol, err := k8s.New(colOpts...)
		if err != nil {
			logger.Error("Failed to initialize Kubernetes collector", "error", err)
			return 1
		}
		apiCfg.K8sCollector = k8sCol
		platformOpts = append(platformOpts, platform.WithK8sCollector(k8sCol))
	}

	// 6. Initialize Shared Discovery Coordinator
	var coordOpts []discovery.Option
	coordOpts = append(coordOpts, discovery.WithLogger(logger))
	if apiCfg.Reconciler != nil {
		coordOpts = append(coordOpts, discovery.WithReconciler(apiCfg.Reconciler))
	}
	if apiCfg.K8sCollector != nil {
		coordOpts = append(coordOpts, discovery.WithCollector(apiCfg.K8sCollector))
	}
	discCoord := discovery.NewCoordinator(coordOpts...)
	apiCfg.Coordinator = discCoord
	platformOpts = append(platformOpts, platform.WithDiscoveryCoordinator(discCoord))

	apiSrv := api.NewServer(apiCfg)
	platformOpts = append(platformOpts, platform.WithAPIServer(apiSrv))

	app := platform.New(cfg, logger, coreClient, platformOpts...)

	startCtx, startCancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer startCancel()

	if err := app.Start(startCtx); err != nil {
		logger.Error("Failed to start platform", "error", err)
		return 1
	}

	// Wait for shutdown signal or context cancellation
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	if testCtx != nil {
		select {
		case sig := <-sigCh:
			logger.Info("Received signal, initiating shutdown", "signal", sig.String())
		case <-testCtx.Done():
			logger.Info("Context cancelled, initiating shutdown")
		}
	} else {
		sig := <-sigCh
		logger.Info("Received signal, initiating shutdown", "signal", sig.String())
	}

	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), config.DefaultShutdownTimeout)
	defer shutdownCancel()

	if err := app.Stop(shutdownCtx); err != nil {
		logger.Error("Shutdown completed with error", "error", err)
		return 1
	}

	logger.Info("WhatBreaks shutdown complete")
	return 0
}

func printUsage(fs *flag.FlagSet, out io.Writer) {
	fmt.Fprintln(out, "Usage: wb [options] [command]")
	fmt.Fprintln(out)
	fmt.Fprintln(out, "Commands:")
	fmt.Fprintln(out, "  help              Show help message")
	fmt.Fprintln(out, "  version           Print WhatBreaks version")
	fmt.Fprintln(out)
	fmt.Fprintln(out, "Options:")
	fs.SetOutput(out)
	fs.PrintDefaults()
}

func detectKubeContext() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	kcPath := filepath.Join(home, ".kube", "config")
	content, err := os.ReadFile(kcPath)
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(content), "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "current-context:") {
			parts := strings.SplitN(line, ":", 2)
			if len(parts) == 2 {
				return strings.Trim(strings.TrimSpace(parts[1]), `"'`)
			}
		}
	}
	return ""
}
