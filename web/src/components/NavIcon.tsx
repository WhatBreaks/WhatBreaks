import React from "react";

export interface NavIconProps {
  size?: number;
  color?: string;
  className?: string;
  style?: React.CSSProperties;
}

/**
 * WhatBreaks official mark (cracked heptagon).
 */
export const NavIcon: React.FC<NavIconProps> = ({
  size = 18,
  color = "currentColor",
  className = "",
  style,
}) => {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill={color}
      className={className}
      style={{ display: "block", flexShrink: 0, ...style }}
      aria-hidden="true"
    >
      {/* Left half */}
      <g transform="translate(2.2,1.2)">
        <path d="M50,0 L50,4 L85.96,21.32 L94.85,60.24 L69.96,91.44 L52,100 L46,74 L56,50 L43,28 Z" />
      </g>
      {/* Right half */}
      <g transform="translate(-2.2,-1.2)">
        <path d="M52,100 L30.04,91.44 L5.15,60.24 L14.04,21.32 L50,0 L43,28 L56,50 L46,74 Z" />
      </g>
    </svg>
  );
};
