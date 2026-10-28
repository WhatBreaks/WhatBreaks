package state_test

import (
	"context"
	"errors"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	corev1 "github.com/aaradhychinche-alt/WhatBreaks/gen/go/wb/core/v1"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/logging"
	"github.com/aaradhychinche-alt/WhatBreaks/internal/state"
	"google.golang.org/grpc"
)

// mockLister implements state.WorkspaceLister.
type mockLister struct {
	mu         sync.Mutex
	workspaces []string
	err        error
}

func (m *mockLister) ListWorkspaces(ctx context.Context) ([]string, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.err != nil {
		return nil, m.err
	}
	out := make([]string, len(m.workspaces))
	copy(out, m.workspaces)
	return out, nil
}

// mockMaterializer implements state.WorkspaceMaterializer.
type mockMaterializer struct {
	mu             sync.Mutex
	materialized   map[string]int
	failWorkspaces map[string]error
	onMaterialize  func(ctx context.Context, ws string)
	callCount      int
}

func newMockMaterializer() *mockMaterializer {
	return &mockMaterializer{
		materialized:   make(map[string]int),
		failWorkspaces: make(map[string]error),
	}
}

func (m *mockMaterializer) Materialize(ctx context.Context, workspaceID string) (*corev1.LoadStateResponse, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	m.mu.Lock()
	m.callCount++
	if fn := m.onMaterialize; fn != nil {
		m.mu.Unlock()
		fn(ctx, workspaceID)
		m.mu.Lock()
	}
	if err, ok := m.failWorkspaces[workspaceID]; ok {
		m.mu.Unlock()
		return nil, err
	}
	m.materialized[workspaceID]++
	m.mu.Unlock()

	return &corev1.LoadStateResponse{
		WorkspaceId:         workspaceID,
		RelationshipsLoaded: 1,
		EvidenceLoaded:      1,
	}, nil
}

// mockLiveness implements state.CoreLivenessChecker.
type mockLiveness struct {
	mu         sync.Mutex
	instanceID string
	err        error
	checkCount int
	onCheck    func(count int) (string, error)
}

func (m *mockLiveness) CheckLiveness(ctx context.Context) (string, error) {
	if err := ctx.Err(); err != nil {
		return "", err
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	m.checkCount++
	if m.onCheck != nil {
		return m.onCheck(m.checkCount)
	}
	if m.err != nil {
		return "", m.err
	}
	return m.instanceID, nil
}

func (m *mockLiveness) SetInstanceID(id string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.instanceID = id
}

func (m *mockLiveness) SetError(err error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.err = err
}

func (m *mockLiveness) SetOnCheck(fn func(count int) (string, error)) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.onCheck = fn
}

// mockReadiness implements state.ReadinessSetter.
type mockReadiness struct {
	ready atomic.Bool
}

func (m *mockReadiness) SetReady(ready bool) {
	m.ready.Store(ready)
}

func (m *mockReadiness) IsReady() bool {
	return m.ready.Load()
}

// ---------------------------------------------------------------------------
// REC-01: Cold boot with existing PostgreSQL data.
// Expected: Persisted workspace graphs are loaded before readiness.
// ---------------------------------------------------------------------------
func TestRecovery_REC01_ColdBootPersistedWorkspacesBeforeReadiness(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1", "ws-2"}}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	// Readiness must be false prior to recovery
	if readiness.IsReady() {
		t.Fatal("expected readiness to be false before recovery")
	}

	ctx := context.Background()
	if err := rm.RecoverAll(ctx); err != nil {
		t.Fatalf("RecoverAll failed: %v", err)
	}

	// Readiness must be true after recovery completes
	if !readiness.IsReady() {
		t.Fatal("expected readiness to be true after recovery")
	}

	// Both workspaces must have been materialized
	mat.mu.Lock()
	if mat.materialized["ws-1"] != 1 || mat.materialized["ws-2"] != 1 {
		t.Errorf("expected both ws-1 and ws-2 to be materialized, got: %+v", mat.materialized)
	}
	mat.mu.Unlock()

	// Both workspaces must be marked ready
	if !rm.IsWorkspaceReady("ws-1") || !rm.IsWorkspaceReady("ws-2") {
		t.Error("expected ws-1 and ws-2 to be ready in RecoveryManager")
	}
}

// ---------------------------------------------------------------------------
// REC-05: Multiple workspaces isolation.
// Expected: Every persisted workspace is restored with strict workspace isolation.
// ---------------------------------------------------------------------------
func TestRecovery_REC05_MultipleWorkspacesIsolation(t *testing.T) {
	workspaces := []string{"tenant-a", "tenant-b", "tenant-c"}
	lister := &mockLister{workspaces: workspaces}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
		state.WithRecoveryConfig(state.RecoveryConfig{
			MaxConcurrency: 2, // test bounded concurrency
			ProbeTimeout:   2 * time.Second,
		}),
	)

	ctx := context.Background()
	if err := rm.RecoverAll(ctx); err != nil {
		t.Fatalf("RecoverAll failed: %v", err)
	}

	mat.mu.Lock()
	for _, ws := range workspaces {
		if mat.materialized[ws] != 1 {
			t.Errorf("expected %s materialized exactly once, got %d", ws, mat.materialized[ws])
		}
	}
	mat.mu.Unlock()

	for _, ws := range workspaces {
		if !rm.IsWorkspaceReady(ws) {
			t.Errorf("expected %s to be ready", ws)
		}
	}
	if rm.IsWorkspaceReady("tenant-unknown") {
		t.Error("unknown workspace must not be marked ready")
	}
}

// ---------------------------------------------------------------------------
// REC-06: Recovery failure.
// Expected: Readiness remains false or affected queries remain gated;
// errors are surfaced rather than ignored.
// ---------------------------------------------------------------------------
func TestRecovery_REC06_RecoveryFailure_ReadinessRemainsFalse(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-ok", "ws-fail"}}
	mat := newMockMaterializer()
	mat.failWorkspaces["ws-fail"] = errors.New("database disk full")
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}
	readiness.SetReady(true) // start ready to verify it transitions to false

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx := context.Background()
	err := rm.RecoverAll(ctx)
	if err == nil {
		t.Fatal("expected RecoverAll to return an error when a workspace fails")
	}

	if readiness.IsReady() {
		t.Fatal("expected readiness to remain false after recovery failure")
	}
}

// ---------------------------------------------------------------------------
// REC-07: PostgreSQL unavailable during recovery.
// Expected: Bounded retry/backoff, clear errors, no false readiness.
// ---------------------------------------------------------------------------
func TestRecovery_REC07_PostgresUnavailable_NoFalseReadiness(t *testing.T) {
	lister := &mockLister{err: errors.New("connection to postgres refused")}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx := context.Background()
	err := rm.RecoverAll(ctx)
	if err == nil {
		t.Fatal("expected RecoverAll to fail when store returns error")
	}
	if !errors.Is(err, lister.err) && !strings.Contains(err.Error(), "connection to postgres refused") {
		t.Fatalf("unexpected error: %v", err)
	}

	if readiness.IsReady() {
		t.Fatal("readiness must be false when postgres is unreachable")
	}
}

// ---------------------------------------------------------------------------
// REC-08: Rust unavailable during recovery.
// Expected: No false readiness; recovery surfaces error.
// ---------------------------------------------------------------------------
func TestRecovery_REC08_RustUnavailable_NoFalseReadiness(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	live := &mockLiveness{err: errors.New("rpc unavailable")}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx := context.Background()
	err := rm.RecoverAll(ctx)
	if err == nil {
		t.Fatal("expected RecoverAll to fail when Rust is unavailable")
	}
	if !errors.Is(err, state.ErrCoreNotAvailable) {
		t.Errorf("expected ErrCoreNotAvailable, got: %v", err)
	}

	if readiness.IsReady() {
		t.Fatal("readiness must be false when Rust is unavailable")
	}
}

// ---------------------------------------------------------------------------
// REC-10: Cancellation and shutdown.
// Expected: Recovery goroutines and retry loops terminate cleanly.
// ---------------------------------------------------------------------------
func TestRecovery_REC10_CancellationAndShutdown(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1", "ws-2"}}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // cancel immediately

	err := rm.RecoverAll(ctx)
	if err == nil {
		t.Fatal("expected RecoverAll to return context cancellation error")
	}
	if !errors.Is(err, context.Canceled) {
		t.Errorf("expected context.Canceled, got: %v", err)
	}

	// Test Stop cleanly terminates monitor
	monitorCtx, monitorCancel := context.WithCancel(context.Background())
	defer monitorCancel()
	rm.StartMonitor(monitorCtx)
	rm.Stop()
	// Should not hang
}

// ---------------------------------------------------------------------------
// REC-11: No persisted workspaces (empty database).
// Expected: Startup completes according to existing empty-database semantics without hanging.
// ---------------------------------------------------------------------------
func TestRecovery_REC11_NoPersistedWorkspaces_EmptyDatabase(t *testing.T) {
	lister := &mockLister{workspaces: []string{}} // zero workspaces
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx := context.Background()
	if err := rm.RecoverAll(ctx); err != nil {
		t.Fatalf("RecoverAll failed on empty database: %v", err)
	}

	if !readiness.IsReady() {
		t.Fatal("expected readiness to be true for empty database")
	}

	mat.mu.Lock()
	if len(mat.materialized) != 0 {
		t.Errorf("expected 0 materializations, got %d", len(mat.materialized))
	}
	mat.mu.Unlock()
}

// ---------------------------------------------------------------------------
// REC-12: Repeated recovery.
// Expected: Idempotent materialization, no duplicate state.
// ---------------------------------------------------------------------------
func TestRecovery_REC12_RepeatedRecovery_Idempotent(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx := context.Background()
	if err := rm.RecoverAll(ctx); err != nil {
		t.Fatalf("first RecoverAll failed: %v", err)
	}
	if err := rm.RecoverAll(ctx); err != nil {
		t.Fatalf("second RecoverAll failed: %v", err)
	}

	mat.mu.Lock()
	if mat.materialized["ws-1"] != 2 {
		t.Errorf("expected ws-1 to be materialized twice, got %d", mat.materialized["ws-1"])
	}
	mat.mu.Unlock()

	if !rm.IsWorkspaceReady("ws-1") {
		t.Error("expected ws-1 to remain ready after repeated recovery")
	}
}

// ---------------------------------------------------------------------------
// REC-02: Automatic recovery after Rust restart.
// Expected: Rust instance ID changes; monitor detects restart and restores workspaces.
// ---------------------------------------------------------------------------
func TestRecovery_REC02_RestartDetectionAndAutomaticRecovery(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-alpha"}}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-A"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
		state.WithRecoveryConfig(state.RecoveryConfig{
			CheckInterval:  20 * time.Millisecond,
			ProbeTimeout:   100 * time.Millisecond,
			InitialBackoff: 10 * time.Millisecond,
		}),
	)

	ctx := context.Background()
	// 1. Initial cold-start recovery into instance-A
	if err := rm.RecoverAll(ctx); err != nil {
		t.Fatalf("initial RecoverAll failed: %v", err)
	}
	if rm.RecoveredInstanceID() != "rust-inst-A" {
		t.Fatalf("expected instance A, got %s", rm.RecoveredInstanceID())
	}

	// 2. Setup recovery hook to observe automatic restart recovery
	recoveryDone := make(chan string, 1)
	rm.SetOnRecoveryComplete(func(newInstanceID string, workspaces []string) {
		recoveryDone <- newInstanceID
	})

	monitorCtx, monitorCancel := context.WithCancel(context.Background())
	defer monitorCancel()
	rm.StartMonitor(monitorCtx)
	defer rm.Stop()

	// 3. Simulate Rust process restart: instance ID changes to "rust-inst-B"
	live.SetInstanceID("rust-inst-B")

	// 4. Wait for monitor to detect restart and complete recovery
	select {
	case newID := <-recoveryDone:
		if newID != "rust-inst-B" {
			t.Errorf("expected recovery into rust-inst-B, got %s", newID)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("timed out waiting for automatic restart recovery")
	}

	if rm.RecoveredInstanceID() != "rust-inst-B" {
		t.Errorf("expected final recovered instance ID rust-inst-B, got %s", rm.RecoveredInstanceID())
	}
	// The completion callback runs before RecoverAll clears its recovering flag.
	// Wait until the recovery cycle has fully returned before checking readiness.
	deadline := time.Now().Add(time.Second)
	for rm.IsRecovering() && time.Now().Before(deadline) {
		time.Sleep(time.Millisecond)
	}
	if rm.IsRecovering() {
		t.Fatal("timed out waiting for restart recovery to finish")
	}
	if !rm.IsWorkspaceReady("ws-alpha") {
		t.Error("expected ws-alpha to be ready after restart recovery")
	}
}

// ---------------------------------------------------------------------------
// REC-09: Concurrent reconciliation and recovery.
// Expected: Existing materializer serialization is respected.
// ---------------------------------------------------------------------------
func TestRecovery_REC09_ConcurrentReconciliationAndRecovery(t *testing.T) {
	memStore := state.NewMemoryStore()
	ctx := context.Background()

	now := time.Now().UTC()
	ws := "ws-concurrent-test"
	_ = memStore.SaveResources(ctx, ws, []state.Resource{
		{Identity: state.ResourceIdentity{Provider: "k8s", ResourceType: "pod", ProviderID: "cluster-1/default/p1"}, FirstObservedAt: now, LastObservedAt: now},
	})

	mockClient := &recordingLoader{}
	mat := state.NewMaterializer(memStore, mockClient, nil)
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		memStore,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	var wg sync.WaitGroup
	errs := make(chan error, 10)

	// Run 5 concurrent RecoverAll and 5 concurrent Materialize calls on the same workspace
	for i := 0; i < 5; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if err := rm.RecoverAll(ctx); err != nil {
				errs <- err
			}
		}()

		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, err := mat.Materialize(ctx, ws); err != nil {
				errs <- err
			}
		}()
	}

	wg.Wait()
	close(errs)

	for err := range errs {
		t.Fatalf("unexpected error during concurrent reconciliation and recovery: %v", err)
	}

	if !readiness.IsReady() {
		t.Error("expected readiness to be true after concurrent recovery operations")
	}
}

type recordingLoader struct {
	mu    sync.Mutex
	calls int
}

func (r *recordingLoader) LoadState(ctx context.Context, req *corev1.LoadStateRequest, opts ...grpc.CallOption) (*corev1.LoadStateResponse, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.calls++
	return &corev1.LoadStateResponse{
		WorkspaceId:         req.WorkspaceId,
		RelationshipsLoaded: uint32(len(req.Relationships)),
	}, nil
}

func (r *recordingLoader) InvalidateWorkspace(ctx context.Context, req *corev1.InvalidateWorkspaceRequest, opts ...grpc.CallOption) (*corev1.InvalidateWorkspaceResponse, error) {
	return &corev1.InvalidateWorkspaceResponse{
		WorkspaceId: req.WorkspaceId,
		Invalidated: true,
	}, nil
}

// ===========================================================================
// FIX 1: TRANSIENT LIVENESS PROBE FAILURE TESTS
// ===========================================================================

func TestRecovery_Fix1_TransientProbeFailureRestoresReadiness(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
		state.WithRecoveryConfig(state.RecoveryConfig{
			CheckInterval:  10 * time.Millisecond,
			ProbeTimeout:   50 * time.Millisecond,
			InitialBackoff: 10 * time.Millisecond,
		}),
	)

	ctx := context.Background()
	// 1. Initial recovery completes into rust-inst-1
	if err := rm.RecoverAll(ctx); err != nil {
		t.Fatalf("RecoverAll failed: %v", err)
	}
	if !readiness.IsReady() {
		t.Fatal("expected readiness true after initial recovery")
	}
	mat.mu.Lock()
	initialCalls := mat.callCount
	mat.mu.Unlock()

	probeFailReported := make(chan struct{})
	probeSuccessReported := make(chan struct{})
	var failOnce, successOnce sync.Once

	// 2. Control probe behavior: first probe fails, next probe succeeds with SAME instance ID
	live.SetOnCheck(func(count int) (string, error) {
		if count == 3 { // probe 1 & 2 were RecoverAll initial & final checks
			failOnce.Do(func() { close(probeFailReported) })
			return "", errors.New("transient network blip")
		}
		if count >= 4 {
			successOnce.Do(func() { close(probeSuccessReported) })
			return "rust-inst-1", nil
		}
		return "rust-inst-1", nil
	})

	monitorCtx, monitorCancel := context.WithCancel(context.Background())
	defer monitorCancel()
	rm.StartMonitor(monitorCtx)
	defer rm.Stop()

	// Wait for failed probe
	select {
	case <-probeFailReported:
	case <-time.After(2 * time.Second):
		t.Fatal("timed out waiting for probe failure")
	}

	// Verify readiness transitioned to false
	deadline := time.Now().Add(500 * time.Millisecond)
	for readiness.IsReady() && time.Now().Before(deadline) {
		time.Sleep(5 * time.Millisecond)
	}
	if readiness.IsReady() {
		t.Fatal("expected readiness to be false after probe failure")
	}

	// Wait for successful probe on same instance
	select {
	case <-probeSuccessReported:
	case <-time.After(2 * time.Second):
		t.Fatal("timed out waiting for probe success")
	}

	// Verify readiness restored to true without re-running RecoverAll
	deadline = time.Now().Add(500 * time.Millisecond)
	for !readiness.IsReady() && time.Now().Before(deadline) {
		time.Sleep(5 * time.Millisecond)
	}
	if !readiness.IsReady() {
		t.Fatal("expected readiness to be restored to true after transient failure")
	}

	mat.mu.Lock()
	callsAfter := mat.callCount
	mat.mu.Unlock()
	if callsAfter != initialCalls {
		t.Errorf("expected no redundant materialization calls, got %d vs initial %d", callsAfter, initialCalls)
	}
}

func TestRecovery_Fix1_MultipleProbeFailuresFollowedByRecovery(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
		state.WithRecoveryConfig(state.RecoveryConfig{
			CheckInterval:  10 * time.Millisecond,
			ProbeTimeout:   50 * time.Millisecond,
			InitialBackoff: 10 * time.Millisecond,
		}),
	)

	ctx := context.Background()
	if err := rm.RecoverAll(ctx); err != nil {
		t.Fatalf("RecoverAll failed: %v", err)
	}

	recoveryDone := make(chan string, 1)
	rm.SetOnRecoveryComplete(func(newInstanceID string, workspaces []string) {
		recoveryDone <- newInstanceID
	})

	// Checks 1 & 2 are startup. Checks 3, 4, 5 fail. Checks 6+ return new instance rust-inst-2
	live.SetOnCheck(func(count int) (string, error) {
		if count >= 3 && count <= 5 {
			return "", errors.New("core unreachable")
		}
		if count > 5 {
			return "rust-inst-2", nil
		}
		return "rust-inst-1", nil
	})

	monitorCtx, monitorCancel := context.WithCancel(context.Background())
	defer monitorCancel()
	rm.StartMonitor(monitorCtx)
	defer rm.Stop()

	select {
	case newID := <-recoveryDone:
		if newID != "rust-inst-2" {
			t.Fatalf("expected recovered instance rust-inst-2, got %s", newID)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("timed out waiting for recovery after consecutive probe failures")
	}

	if !readiness.IsReady() {
		t.Fatal("expected readiness to be true after successful recovery into new instance")
	}
	if rm.RecoveredInstanceID() != "rust-inst-2" {
		t.Errorf("expected RecoveredInstanceID rust-inst-2, got %s", rm.RecoveredInstanceID())
	}
}

func TestRecovery_Fix1_ProbeFailureDuringStartup(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	live := &mockLiveness{err: errors.New("rust server offline")}
	readiness := &mockReadiness{}
	readiness.SetReady(true) // start true to ensure it is reset to false

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx := context.Background()
	err := rm.RecoverAll(ctx)
	if err == nil {
		t.Fatal("expected RecoverAll to fail on startup probe failure")
	}
	if !errors.Is(err, state.ErrCoreNotAvailable) {
		t.Errorf("expected ErrCoreNotAvailable, got: %v", err)
	}
	if readiness.IsReady() {
		t.Fatal("readiness must be false when startup probe fails")
	}
	if rm.IsRecoveryComplete() {
		t.Fatal("recovery must not be marked complete when startup probe fails")
	}
}

func TestRecovery_Fix1_InstanceIDChangeFollowedByFailedRecovery(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
		state.WithRecoveryConfig(state.RecoveryConfig{
			CheckInterval:  10 * time.Millisecond,
			ProbeTimeout:   50 * time.Millisecond,
			InitialBackoff: 10 * time.Millisecond,
		}),
	)

	ctx := context.Background()
	if err := rm.RecoverAll(ctx); err != nil {
		t.Fatalf("initial RecoverAll failed: %v", err)
	}

	// Setup failure for the new instance
	mat.failWorkspaces["ws-1"] = errors.New("corrupt workspace graph")

	failureReported := make(chan error, 1)
	rm.SetOnRecoveryFailure(func(err error) {
		select {
		case failureReported <- err:
		default:
		}
	})

	monitorCtx, monitorCancel := context.WithCancel(context.Background())
	defer monitorCancel()
	rm.StartMonitor(monitorCtx)
	defer rm.Stop()

	// Instance changes to rust-inst-2
	live.SetInstanceID("rust-inst-2")

	select {
	case <-failureReported:
	case <-time.After(3 * time.Second):
		t.Fatal("timed out waiting for recovery failure report")
	}

	if readiness.IsReady() {
		t.Fatal("readiness must remain false when recovery of new instance fails")
	}
	if rm.IsRecoveryComplete() {
		t.Fatal("recovery must not be marked complete when recovery fails")
	}
	if rm.IsWorkspaceReady("ws-1") {
		t.Fatal("workspace ws-1 must not be ready when recovery fails")
	}
}

func TestRecovery_Fix1_TransientProbeFailureWhileRecoveryIncomplete(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	mat.failWorkspaces["ws-1"] = errors.New("db down")
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
		state.WithRecoveryConfig(state.RecoveryConfig{
			CheckInterval:  10 * time.Millisecond,
			ProbeTimeout:   50 * time.Millisecond,
			InitialBackoff: 20 * time.Millisecond,
		}),
	)

	// Cold start recovery fails
	_ = rm.RecoverAll(context.Background())
	if readiness.IsReady() || rm.IsRecoveryComplete() {
		t.Fatal("precondition failed: recovery should be incomplete")
	}

	// Probe fails once, then succeeds with rust-inst-1
	live.SetOnCheck(func(count int) (string, error) {
		if count == 3 {
			return "", errors.New("temporary probe error")
		}
		return "rust-inst-1", nil
	})

	monitorCtx, monitorCancel := context.WithCancel(context.Background())
	defer monitorCancel()
	rm.StartMonitor(monitorCtx)
	defer rm.Stop()

	// Wait briefly for a few monitor iterations
	time.Sleep(100 * time.Millisecond)

	// Because recovery never completed, successful probe alone must NEVER set readiness true
	if readiness.IsReady() {
		t.Fatal("readiness must NOT be restored by probe success when recovery is incomplete")
	}
	if rm.IsRecoveryComplete() {
		t.Fatal("recoveryComplete must remain false")
	}
}

// ===========================================================================
// FIX 2: VALIDATE SERVER INSTANCE AFTER RECOVERY TESTS
// ===========================================================================

func TestRecovery_Fix2_InstanceChangesBetweenInitialAndFinalProbe(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	live := &mockLiveness{}
	readiness := &mockReadiness{}
	readiness.SetReady(true)

	// Probe 1: rust-inst-1 (initial probe)
	// Probe 2: rust-inst-2 (final validation probe)
	live.SetOnCheck(func(count int) (string, error) {
		if count == 1 {
			return "rust-inst-1", nil
		}
		return "rust-inst-2", nil
	})

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx := context.Background()
	err := rm.RecoverAll(ctx)
	if err == nil {
		t.Fatal("expected RecoverAll to fail when instance ID changes between initial and final probe")
	}
	if !strings.Contains(err.Error(), "restarted during recovery") {
		t.Fatalf("expected restart error message, got: %v", err)
	}

	if readiness.IsReady() {
		t.Fatal("readiness must remain false when instance changed during recovery")
	}
	if rm.IsRecoveryComplete() {
		t.Fatal("recoveryComplete must be false")
	}
	if rm.IsWorkspaceReady("ws-1") {
		t.Fatal("workspace ws-1 must not be marked ready")
	}
}

func TestRecovery_Fix2_RustUnavailableAtFinalProbe(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	live := &mockLiveness{}
	readiness := &mockReadiness{}

	live.SetOnCheck(func(count int) (string, error) {
		if count == 1 {
			return "rust-inst-1", nil
		}
		return "", errors.New("rust process crashed")
	})

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx := context.Background()
	err := rm.RecoverAll(ctx)
	if err == nil {
		t.Fatal("expected RecoverAll to fail when Rust is unavailable at final probe")
	}

	if readiness.IsReady() {
		t.Fatal("readiness must remain false")
	}
	if rm.IsRecoveryComplete() {
		t.Fatal("recoveryComplete must be false")
	}
}

func TestRecovery_Fix2_InstanceChangesWhileMultipleWorkersRunning(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1", "ws-2", "ws-3"}}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-A"}
	readiness := &mockReadiness{}

	mat.onMaterialize = func(ctx context.Context, ws string) {
		if ws == "ws-2" {
			// Rust process restarts while workers are executing
			live.SetInstanceID("rust-inst-B")
		}
	}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
		state.WithRecoveryConfig(state.RecoveryConfig{
			MaxConcurrency: 2,
			ProbeTimeout:   2 * time.Second,
		}),
	)

	ctx := context.Background()
	err := rm.RecoverAll(ctx)
	if err == nil {
		t.Fatal("expected RecoverAll to detect instance change mid-recovery")
	}
	if !strings.Contains(err.Error(), "restarted during recovery") {
		t.Fatalf("expected restarted error, got: %v", err)
	}

	if readiness.IsReady() {
		t.Fatal("readiness must be false")
	}
	if rm.IsRecoveryComplete() {
		t.Fatal("recoveryComplete must be false")
	}
}

func TestRecovery_Fix2_RecoveryRetriesSuccessfullyAgainstStableNewInstance(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	live := &mockLiveness{}
	readiness := &mockReadiness{}

	// Attempt 1: check 1 = "inst-A", check 2 = "inst-B" (fails)
	// Attempt 2: check 3 = "inst-B", check 4 = "inst-B" (succeeds)
	live.SetOnCheck(func(count int) (string, error) {
		switch count {
		case 1:
			return "inst-A", nil
		case 2:
			return "inst-B", nil
		default:
			return "inst-B", nil
		}
	})

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx := context.Background()
	err1 := rm.RecoverAll(ctx)
	if err1 == nil {
		t.Fatal("expected first RecoverAll to fail due to instance change")
	}
	if readiness.IsReady() {
		t.Fatal("readiness must be false after failed attempt")
	}

	// Retry against stable new instance
	err2 := rm.RecoverAll(ctx)
	if err2 != nil {
		t.Fatalf("expected second RecoverAll to succeed, got: %v", err2)
	}
	if !readiness.IsReady() {
		t.Fatal("readiness must be true after successful retry")
	}
	if !rm.IsRecoveryComplete() {
		t.Fatal("recoveryComplete must be true")
	}
	if rm.RecoveredInstanceID() != "inst-B" {
		t.Errorf("expected RecoveredInstanceID inst-B, got %s", rm.RecoveredInstanceID())
	}
}

func TestRecovery_Fix2_EmptyDatabase_InstanceChangesAtFinalProbe(t *testing.T) {
	lister := &mockLister{workspaces: []string{}}
	mat := newMockMaterializer()
	live := &mockLiveness{}
	readiness := &mockReadiness{}

	live.SetOnCheck(func(count int) (string, error) {
		if count == 1 {
			return "inst-1", nil
		}
		return "inst-2", nil
	})

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	ctx := context.Background()
	err := rm.RecoverAll(ctx)
	if err == nil {
		t.Fatal("expected empty-database recovery to fail when instance changes at final probe")
	}
	if readiness.IsReady() {
		t.Fatal("readiness must be false")
	}
	if rm.IsRecoveryComplete() {
		t.Fatal("recoveryComplete must be false")
	}
}

// ===========================================================================
// FIX 4: CANCEL RECOVERY ON SHUTDOWN TESTS
// ===========================================================================

func TestRecovery_Fix4_DeterministicCancellationAndRepeatedStop(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-1"}}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
	)

	monitorCtx, monitorCancel := context.WithCancel(context.Background())
	defer monitorCancel()
	rm.StartMonitor(monitorCtx)

	// Call Stop from 10 concurrent goroutines
	var wg sync.WaitGroup
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			rm.Stop()
		}()
	}
	wg.Wait()

	// Call Stop again sequentially (should not panic or hang)
	rm.Stop()
}

func TestRecovery_Fix4_InFlightRecoveryCancelledPromptlyOnShutdown(t *testing.T) {
	lister := &mockLister{workspaces: []string{"ws-blocked"}}
	mat := newMockMaterializer()
	live := &mockLiveness{instanceID: "rust-inst-1"}
	readiness := &mockReadiness{}

	matStarted := make(chan struct{})
	mat.onMaterialize = func(ctx context.Context, ws string) {
		close(matStarted)
		<-ctx.Done() // Block until context is cancelled by Stop()
	}

	rm := state.NewRecoveryManager(
		lister,
		mat,
		live,
		readiness,
		logging.NewStandardLogger(nil, logging.LevelDebug),
		state.WithRecoveryConfig(state.RecoveryConfig{
			CheckInterval: 10 * time.Millisecond,
		}),
	)

	monitorCtx, monitorCancel := context.WithCancel(context.Background())
	defer monitorCancel()
	rm.StartMonitor(monitorCtx)

	// Wait for worker goroutine to start
	select {
	case <-matStarted:
	case <-time.After(2 * time.Second):
		t.Fatal("timed out waiting for materialization worker to start")
	}

	// Trigger Stop while worker is in flight
	stopDone := make(chan struct{})
	go func() {
		rm.Stop()
		close(stopDone)
	}()

	select {
	case <-stopDone:
		// Promptly stopped
	case <-time.After(2 * time.Second):
		t.Fatal("rm.Stop() took too long to cancel in-flight recovery")
	}

	if readiness.IsReady() {
		t.Fatal("readiness must be false after cancelled recovery")
	}
	if rm.IsRecoveryComplete() {
		t.Fatal("recoveryComplete must be false after cancelled recovery")
	}
}
