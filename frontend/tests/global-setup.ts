// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

import { execSync } from 'child_process';
import { join } from 'path';

const PROJECT_ROOT = join(__dirname, '..', '..');

/**
 * Global setup: Build the Rust service and Go client before tests run.
 */
async function globalSetup() {
  console.log('Building Rust service...');
  try {
    execSync('cargo build --release', {
      cwd: PROJECT_ROOT,
      stdio: 'inherit',
    });
    console.log('Rust service built successfully.');
  } catch (error) {
    console.error('Failed to build Rust service:', error);
    throw error;
  }

  console.log('Building Go writer client...');
  try {
    execSync('go build -o cxdb-writer .', {
      cwd: join(PROJECT_ROOT, 'examples', 'basic-go'),
      stdio: 'inherit',
    });
    console.log('Go writer built successfully.');
  } catch (error) {
    console.error('Failed to build Go writer:', error);
    throw error;
  }

  // Verify binaries exist
  const serverBinary = join(PROJECT_ROOT, 'target', 'release', 'cxdb-server');
  const writerBinary = join(PROJECT_ROOT, 'examples', 'basic-go', 'cxdb-writer');

  try {
    execSync(`test -f "${serverBinary}"`, { stdio: 'ignore' });
  } catch {
    throw new Error(`Rust server binary not found at ${serverBinary}`);
  }

  try {
    execSync(`test -f "${writerBinary}"`, { stdio: 'ignore' });
  } catch {
    throw new Error(`Go writer binary not found at ${writerBinary}`);
  }

  console.log('All binaries verified. Ready for tests.');
}

export default globalSetup;
