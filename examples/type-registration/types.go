// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

package main

import "time"

// LogEntry represents a structured log message with metadata.
//
// Uses msgpack numeric tags for forward-compatible schema evolution.
// Tags are immutable once assigned - never reuse or change a tag number.
type LogEntry struct {
	// Timestamp is unix milliseconds (semantic: unix_ms for UI rendering)
	Timestamp uint64 `msgpack:"1"`

	// Level is 0=DEBUG, 1=INFO, 2=WARN, 3=ERROR
	Level uint8 `msgpack:"2"`

	// Message is the log text
	Message string `msgpack:"3"`

	// Tags are arbitrary key-value metadata
	Tags map[string]string `msgpack:"4"`
}

// NewLogEntry creates a LogEntry with the current timestamp.
func NewLogEntry(level uint8, message string, tags map[string]string) *LogEntry {
	return &LogEntry{
		Timestamp: uint64(time.Now().UnixMilli()),
		Level:     level,
		Message:   message,
		Tags:      tags,
	}
}

// Log levels
const (
	LevelDebug uint8 = 0
	LevelInfo  uint8 = 1
	LevelWarn  uint8 = 2
	LevelError uint8 = 3
)

// LevelName returns the string name for a level.
func LevelName(level uint8) string {
	switch level {
	case LevelDebug:
		return "DEBUG"
	case LevelInfo:
		return "INFO"
	case LevelWarn:
		return "WARN"
	case LevelError:
		return "ERROR"
	default:
		return "UNKNOWN"
	}
}
