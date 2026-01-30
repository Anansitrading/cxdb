// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

/**
 * LogEntry Renderer
 *
 * A custom renderer for com.example.LogEntry types that displays
 * structured log data with syntax highlighting and level-based styling.
 *
 * This renderer demonstrates:
 * - Props interface (data, metadata, theme)
 * - Conditional styling based on data
 * - Pretty-printed JSON display
 * - Theme-aware rendering
 */

import React from 'react';

// Map log levels to colors
const LEVEL_COLORS = {
  DEBUG: { light: '#6b7280', dark: '#9ca3af' },
  INFO: { light: '#3b82f6', dark: '#60a5fa' },
  WARN: { light: '#f59e0b', dark: '#fbbf24' },
  ERROR: { light: '#ef4444', dark: '#f87171' },
};

// Map log levels to background colors
const LEVEL_BG = {
  DEBUG: { light: '#f3f4f6', dark: '#1f2937' },
  INFO: { light: '#eff6ff', dark: '#1e3a8a' },
  WARN: { light: '#fffbeb', dark: '#78350f' },
  ERROR: { light: '#fef2f2', dark: '#7f1d1d' },
};

/**
 * LogEntryRenderer component
 *
 * @param {object} props
 * @param {object} props.data - Typed turn data (projected from msgpack)
 * @param {string} props.data.timestamp - ISO-8601 timestamp (rendered by semantic hint)
 * @param {string} props.data.level - Log level enum label (DEBUG, INFO, WARN, ERROR)
 * @param {string} props.data.message - Log message text
 * @param {object} props.data.tags - Optional key-value metadata
 * @param {object} props.metadata - Turn metadata (turn_id, depth, declared_type)
 * @param {string} props.theme - UI theme ('light' or 'dark')
 * @param {function} props.onError - Optional error callback
 */
export default function LogEntryRenderer({ data, metadata, theme, onError }) {
  // Validate input
  if (!data || typeof data !== 'object') {
    return (
      <div style={{ color: 'red', padding: '1rem' }}>
        Invalid data: expected object
      </div>
    );
  }

  const { timestamp, level, message, tags } = data;
  const isDark = theme === 'dark';

  // Get colors for this log level
  const levelColor = LEVEL_COLORS[level]?.[theme] || (isDark ? '#9ca3af' : '#6b7280');
  const levelBg = LEVEL_BG[level]?.[theme] || (isDark ? '#1f2937' : '#f3f4f6');
  const textColor = isDark ? '#e5e7eb' : '#1f2937';
  const tagBg = isDark ? '#374151' : '#f9fafb';
  const tagBorder = isDark ? '#4b5563' : '#e5e7eb';

  return (
    <div
      style={{
        fontFamily: 'ui-monospace, SFMono-Regular, Monaco, Consolas, monospace',
        fontSize: '0.875rem',
        lineHeight: '1.5',
        padding: '1rem',
        borderRadius: '0.5rem',
        backgroundColor: levelBg,
        border: `1px solid ${levelColor}`,
      }}
    >
      {/* Header: Level + Timestamp */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '0.75rem',
          marginBottom: '0.75rem',
        }}
      >
        <span
          style={{
            display: 'inline-block',
            padding: '0.25rem 0.5rem',
            borderRadius: '0.25rem',
            fontWeight: 'bold',
            fontSize: '0.75rem',
            color: '#fff',
            backgroundColor: levelColor,
          }}
        >
          {level}
        </span>
        <span style={{ color: isDark ? '#9ca3af' : '#6b7280', fontSize: '0.75rem' }}>
          {timestamp}
        </span>
        <span style={{ color: isDark ? '#6b7280' : '#9ca3af', fontSize: '0.75rem' }}>
          Turn {metadata?.turn_id}
        </span>
      </div>

      {/* Message */}
      <div
        style={{
          color: textColor,
          marginBottom: tags && Object.keys(tags).length > 0 ? '0.75rem' : 0,
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-word',
        }}
      >
        {message}
      </div>

      {/* Tags (if present) */}
      {tags && Object.keys(tags).length > 0 && (
        <div
          style={{
            display: 'flex',
            flexWrap: 'wrap',
            gap: '0.5rem',
            marginTop: '0.75rem',
          }}
        >
          {Object.entries(tags).map(([key, value]) => (
            <div
              key={key}
              style={{
                display: 'flex',
                alignItems: 'center',
                padding: '0.25rem 0.5rem',
                borderRadius: '0.25rem',
                backgroundColor: tagBg,
                border: `1px solid ${tagBorder}`,
                fontSize: '0.75rem',
              }}
            >
              <span style={{ color: levelColor, fontWeight: '500' }}>{key}:</span>
              <span style={{ color: textColor, marginLeft: '0.25rem' }}>{value}</span>
            </div>
          ))}
        </div>
      )}

      {/* Debug info (collapsible) */}
      <details style={{ marginTop: '1rem' }}>
        <summary
          style={{
            cursor: 'pointer',
            color: isDark ? '#9ca3af' : '#6b7280',
            fontSize: '0.75rem',
            userSelect: 'none',
          }}
        >
          Raw data
        </summary>
        <pre
          style={{
            marginTop: '0.5rem',
            padding: '0.5rem',
            backgroundColor: isDark ? '#111827' : '#f9fafb',
            border: `1px solid ${tagBorder}`,
            borderRadius: '0.25rem',
            fontSize: '0.75rem',
            overflow: 'auto',
            color: textColor,
          }}
        >
          {JSON.stringify(data, null, 2)}
        </pre>
      </details>
    </div>
  );
}
