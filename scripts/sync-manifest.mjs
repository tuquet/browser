#!/usr/bin/env node

/**
 * Tuquet Browser - Upstream Release Watcher & Manifest Sync
 *
 * Polls https://github.com/adryfish/fingerprint-chromium/releases
 * and synchronizes release metadata into manifest.json.
 *
 * Preserves curated statuses (recommended, buggy, notes) while
 * automatically discovering new upstream releases and asset URLs.
 */

import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const MANIFEST_PATH = path.resolve(__dirname, '..', 'manifest.json');

const UPSTREAM_REPO = 'adryfish/fingerprint-chromium';
const GITHUB_API_URL = `https://api.github.com/repos/${UPSTREAM_REPO}/releases?per_page=15`;

async function fetchReleases() {
  const headers = {
    'User-Agent': 'Tuquet-Release-Watcher/1.0',
    'Accept': 'application/vnd.github.v3+json',
  };

  if (process.env.GITHUB_TOKEN) {
    headers['Authorization'] = `token ${process.env.GITHUB_TOKEN}`;
  }

  try {
    const res = await fetch(GITHUB_API_URL, { headers });
    if (res.ok) {
      return await res.json();
    }
  } catch (err) {
    // If native fetch fails due to Windows corporate proxy / self-signed TLS certs, fallback to curl.exe
  }

  // Fallback via curl
  try {
    const tokenHeader = process.env.GITHUB_TOKEN ? `-H "Authorization: token ${process.env.GITHUB_TOKEN}"` : '';
    const cmd = `curl.exe -s -H "User-Agent: Tuquet-Release-Watcher/1.0" ${tokenHeader} "${GITHUB_API_URL}"`;
    const output = execSync(cmd, { encoding: 'utf-8', stdio: ['pipe', 'pipe', 'ignore'] });
    return JSON.parse(output);
  } catch (err) {
    throw new Error(`Failed to fetch releases via both fetch and curl: ${err.message}`);
  }
}

async function main() {
  const args = process.argv.slice(2);
  const isDryRun = args.includes('--dry-run');

  if (!fs.existsSync(MANIFEST_PATH)) {
    console.error(`Error: Manifest not found at ${MANIFEST_PATH}`);
    process.exit(1);
  }

  const currentManifest = JSON.parse(fs.readFileSync(MANIFEST_PATH, 'utf-8'));
  console.log(`Loaded manifest: ${currentManifest.releases.length} existing releases.`);

  console.log(`Fetching upstream releases from ${UPSTREAM_REPO}...`);
  const upstreamReleases = await fetchReleases();
  console.log(`Discovered ${upstreamReleases.length} releases upstream.`);

  let changesCount = 0;
  const existingMap = new Map();
  for (const rel of currentManifest.releases) {
    existingMap.set(rel.version, rel);
  }

  const updatedReleases = [];

  for (const up of upstreamReleases) {
    const tag = up.tag_name.replace(/^v/, '');
    const existing = existingMap.get(tag);

    // Extract platform assets
    const platforms = {};
    for (const asset of up.assets || []) {
      const name = asset.name;
      if (name.endsWith('_windows_x64.zip') || name.includes('win64') || name.includes('windows_x64.zip')) {
        platforms.win64 = {
          asset_name: name,
          url: asset.browser_download_url,
          size: asset.size,
        };
      } else if (name.endsWith('_linux.tar.xz') || name.endsWith('linux_x64.tar.xz')) {
        platforms.linux64 = {
          asset_name: name,
          url: asset.browser_download_url,
          size: asset.size,
        };
      } else if (name.endsWith('_macos.dmg') || name.includes('macos')) {
        platforms['mac-x64'] = {
          asset_name: name,
          url: asset.browser_download_url,
          size: asset.size,
        };
      }
    }

    if (existing) {
      // Merge assets while strictly preserving curated status & notes
      const mergedPlatforms = { ...existing.platforms, ...platforms };
      // Preserve existing sha256 checksums if already calculated
      for (const [pk, pv] of Object.entries(existing.platforms || {})) {
        if (pv.sha256 && mergedPlatforms[pk]) {
          mergedPlatforms[pk].sha256 = pv.sha256;
        }
      }

      updatedReleases.push({
        ...existing,
        published_at: up.published_at || existing.published_at,
        platforms: mergedPlatforms,
      });
    } else {
      console.log(`  ★ New release detected upstream: v${tag}`);
      changesCount++;
      updatedReleases.push({
        version: tag,
        tag: tag,
        channel: 'latest',
        status: 'unverified',
        notes: `Newly discovered upstream release ${tag}. Under evaluation by Tuquet team.`,
        published_at: up.published_at,
        platforms,
      });
    }
  }

  // Preserve any older releases that were in manifest but not in the latest 15 releases
  for (const rel of currentManifest.releases) {
    if (!updatedReleases.some((r) => r.version === rel.version)) {
      updatedReleases.push(rel);
    }
  }

  // Determine latest channel
  const latestRelease = updatedReleases[0];
  if (latestRelease && latestRelease.version !== currentManifest.channels.latest) {
    currentManifest.channels.latest = latestRelease.version;
    changesCount++;
  }

  currentManifest.updated_at = new Date().toISOString();
  currentManifest.releases = updatedReleases;

  if (changesCount > 0) {
    console.log(`Changes detected: ${changesCount} update(s).`);
    if (!isDryRun) {
      fs.writeFileSync(MANIFEST_PATH, JSON.stringify(currentManifest, null, 2) + '\n');
      console.log(`Successfully updated ${MANIFEST_PATH}`);
      if (process.env.GITHUB_OUTPUT) {
        fs.appendFileSync(process.env.GITHUB_OUTPUT, `has_changes=true\nnew_version=${latestRelease?.version || ''}\n`);
      }
    } else {
      console.log('[Dry Run] Manifest updated in memory. Skipped file write.');
    }
  } else {
    console.log('No new releases or metadata changes found. Manifest is 100% up to date.');
    if (process.env.GITHUB_OUTPUT) {
      fs.appendFileSync(process.env.GITHUB_OUTPUT, `has_changes=false\n`);
    }
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
