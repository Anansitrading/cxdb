'use client';

import { cn } from '@/lib/utils';
import { AlertTriangle, AlertOctagon } from '@/components/icons';
import type { SessionMetrics, ErrorMetrics, PerfMetrics } from '@/types';

export interface SessionsErrorsBarProps {
  sessions: SessionMetrics;
  errors: ErrorMetrics;
  perf: PerfMetrics;
  className?: string;
}

export function SessionsErrorsBar({ sessions, errors, perf, className }: SessionsErrorsBarProps) {
  const errorRate = perf.http_errors_tps_1m;
  const isElevated = errorRate > 0.5;
  const isHigh = errorRate > 2.0;

  // Format error breakdown
  const errorBreakdown = Object.entries(errors.by_type)
    .map(([type, count]) => `${count} ${type}`)
    .join(', ');

  return (
    <div
      className={cn(
        'bg-theme-bg-tertiary/50 border border-theme-border rounded-lg px-4 py-2 flex items-center justify-between flex-wrap gap-2',
        className
      )}
    >
      {/* Sessions info */}
      <div className="flex items-center gap-4 text-sm">
        <span className="text-theme-text-secondary">
          Sessions:{' '}
          <span className="text-emerald-400 font-medium">{sessions.active}</span>
          {' active'}
          {sessions.idle > 0 && (
            <span className="text-theme-text-dim">, {sessions.idle} idle</span>
          )}
          <span className="text-theme-text-dim"> ({sessions.total.toLocaleString()} total)</span>
        </span>
      </div>

      {/* Divider */}
      <div className="hidden sm:block w-px h-4 bg-theme-bg-hover" />

      {/* Errors info */}
      <div className="flex items-center gap-2 text-sm">
        <span className="text-theme-text-secondary">
          Errors: <span className="text-theme-text-muted">{errors.total}</span>
          {errorBreakdown && (
            <span className="text-theme-text-dim"> ({errorBreakdown})</span>
          )}
        </span>

        {/* Error rate indicator */}
        {isHigh && (
          <span className="flex items-center gap-1 px-2 py-0.5 bg-red-900/30 border border-red-500/50 rounded text-red-400 text-xs">
            <AlertOctagon className="w-3 h-3" />
            {errorRate.toFixed(1)}/s (high)
          </span>
        )}
        {isElevated && !isHigh && (
          <span className="flex items-center gap-1 px-2 py-0.5 bg-amber-900/30 border border-amber-500/50 rounded text-amber-400 text-xs">
            <AlertTriangle className="w-3 h-3" />
            {errorRate.toFixed(1)}/s (elevated)
          </span>
        )}
      </div>
    </div>
  );
}
