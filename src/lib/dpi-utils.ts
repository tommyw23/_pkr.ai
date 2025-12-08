// src/lib/dpi-utils.ts
// Utility functions for handling DPI scaling in Tauri windows

import { invoke } from "@tauri-apps/api/core";

/**
 * Get the DPI scale factor from the backend
 * Falls back to window.devicePixelRatio if backend call fails
 *
 * @returns Promise<number> - The DPI scale factor (e.g., 1.0, 1.5, 2.0)
 */
export async function getDpiScaleFactor(): Promise<number> {
  try {
    const scaleFactor = await invoke<number>("get_dpi_scale_factor");
    console.log(`🔍 Backend DPI scale factor: ${scaleFactor}`);
    return scaleFactor;
  } catch (error) {
    console.warn("Failed to get DPI scale factor from backend, using window.devicePixelRatio:", error);
    return window.devicePixelRatio || 1.0;
  }
}

/**
 * Convert logical coordinates to physical coordinates
 * Logical coords are what the browser/Tauri window uses
 * Physical coords are what screenshot capture needs
 *
 * @param x - Logical x coordinate
 * @param y - Logical y coordinate
 * @param width - Logical width
 * @param height - Logical height
 * @param scaleFactor - DPI scale factor (from getDpiScaleFactor)
 * @returns Physical coordinates object
 */
export function logicalToPhysical(
  x: number,
  y: number,
  width: number,
  height: number,
  scaleFactor: number
): { x: number; y: number; width: number; height: number } {
  return {
    x: Math.round(Math.max(0, x) * scaleFactor),
    y: Math.round(Math.max(0, y) * scaleFactor),
    width: Math.round(width * scaleFactor),
    height: Math.round(height * scaleFactor),
  };
}

/**
 * Convert physical coordinates back to logical coordinates
 *
 * @param x - Physical x coordinate
 * @param y - Physical y coordinate
 * @param width - Physical width
 * @param height - Physical height
 * @param scaleFactor - DPI scale factor (from getDpiScaleFactor)
 * @returns Logical coordinates object
 */
export function physicalToLogical(
  x: number,
  y: number,
  width: number,
  height: number,
  scaleFactor: number
): { x: number; y: number; width: number; height: number } {
  return {
    x: Math.round(x / scaleFactor),
    y: Math.round(y / scaleFactor),
    width: Math.round(width / scaleFactor),
    height: Math.round(height / scaleFactor),
  };
}

/**
 * Get both browser and backend DPI scale factors for debugging
 * Useful for diagnosing DPI detection issues
 *
 * @returns Promise with both scale factors
 */
export async function getDpiScaleFactors(): Promise<{
  browser: number;
  backend: number;
  match: boolean;
}> {
  const browser = window.devicePixelRatio || 1.0;
  const backend = await getDpiScaleFactor();

  return {
    browser,
    backend,
    match: Math.abs(browser - backend) < 0.01,
  };
}
