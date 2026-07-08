#!/usr/bin/env node
/**
 * bump-version.js — 一键同步所有版本号
 *
 * 用法:
 *   node scripts/bump-version.js 0.2.0        # 指定版本
 *   node scripts/bump-version.js patch         # 0.1.0 → 0.1.1
 *   node scripts/bump-version.js minor         # 0.1.0 → 0.2.0
 *   node scripts/bump-version.js major         # 0.1.0 → 1.0.0
 *   node scripts/bump-version.js patch --tag   # bump + git commit + tag
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const ROOT = path.resolve(__dirname, '..');

// ── 版本号文件路径 ──
const targets = {
  'package.json':         path.join(ROOT, 'package.json'),
  'package-lock.json':    path.join(ROOT, 'package-lock.json'),
  'Cargo.toml':           path.join(ROOT, 'src-tauri', 'Cargo.toml'),
  'tauri.conf.json':      path.join(ROOT, 'src-tauri', 'tauri.conf.json'),
  'index.html':           path.join(ROOT, 'src', 'index.html'),
};

// ── 读取当前版本 ──
function getCurrentVersion() {
  const pkg = JSON.parse(fs.readFileSync(targets['package.json'], 'utf8'));
  return pkg.version;
}

// ── 计算新版本 ──
function calcNewVersion(current, input) {
  if (/^\d+\.\d+\.\d+$/.test(input)) return input;

  const [major, minor, patch] = current.split('.').map(Number);
  switch (input) {
    case 'major': return `${major + 1}.0.0`;
    case 'minor': return `${major}.${minor + 1}.0`;
    case 'patch': return `${major}.${minor}.${patch + 1}`;
    default:
      console.error(`❌ 无效的版本参数: "${input}"`);
      console.error('   用法: node scripts/bump-version.js <patch|minor|major|x.y.z>');
      process.exit(1);
  }
}

// ── 更新各文件 ──
function updatePackageJson(version) {
  const file = targets['package.json'];
  const pkg = JSON.parse(fs.readFileSync(file, 'utf8'));
  pkg.version = version;
  fs.writeFileSync(file, JSON.stringify(pkg, null, 2) + '\n');
  console.log(`  ✅ package.json`);
}

function updatePackageLockJson(version) {
  const file = targets['package-lock.json'];
  if (!fs.existsSync(file)) {
    console.log(`  ⏭️  package-lock.json (不存在，跳过)`);
    return;
  }
  const lock = JSON.parse(fs.readFileSync(file, 'utf8'));
  lock.version = version;
  if (lock.packages && lock.packages['']) {
    lock.packages[''].version = version;
  }
  fs.writeFileSync(file, JSON.stringify(lock, null, 2) + '\n');
  console.log(`  ✅ package-lock.json`);
}

function updateCargoToml(version) {
  const file = targets['Cargo.toml'];
  let content = fs.readFileSync(file, 'utf8');
  content = content.replace(
    /^version\s*=\s*".*"$/m,
    `version = "${version}"`
  );
  fs.writeFileSync(file, content);
  console.log(`  ✅ Cargo.toml`);
}

function updateTauriConf(version) {
  const file = targets['tauri.conf.json'];
  const conf = JSON.parse(fs.readFileSync(file, 'utf8'));
  conf.version = version;
  fs.writeFileSync(file, JSON.stringify(conf, null, 2) + '\n');
  console.log(`  ✅ tauri.conf.json`);
}

function updateIndexHtml(version) {
  const file = targets['index.html'];
  let content = fs.readFileSync(file, 'utf8');
  const oldVersion = getCurrentVersion();
  // 只替换 settings 区域中的版本号（精确匹配 setting-value 旁边的版本）
  const regex = /(<span class="setting-value">)\d+\.\d+\.\d+(<\/span>)/;
  if (regex.test(content)) {
    content = content.replace(regex, `$1${version}$2`);
    fs.writeFileSync(file, content);
    console.log(`  ✅ index.html (settings UI)`);
  } else {
    console.log(`  ⚠️  index.html (未找到版本号标记，跳过)`);
  }
}

// ── Git 操作 ──
function gitCommitAndTag(version, doTag) {
  const files = Object.values(targets);
  try {
    for (const f of files) {
      execSync(`git add "${f}"`, { cwd: ROOT, stdio: 'pipe' });
    }
    execSync(`git commit -m "release: v${version}"`, { cwd: ROOT, stdio: 'inherit' });
    console.log(`\n  📦 git commit: "release: v${version}"`);

    if (doTag) {
      execSync(`git tag v${version}`, { cwd: ROOT, stdio: 'inherit' });
      console.log(`  🏷️  git tag: v${version}`);
    }
  } catch (e) {
    console.error(`\n  ⚠️  Git 操作失败: ${e.message}`);
    console.error('   版本文件已更新，请手动 commit。');
  }
}

// ── 主流程 ──
const args = process.argv.slice(2);
const doTag = args.includes('--tag');
const versionArg = args.filter(a => !a.startsWith('--'))[0];

if (!versionArg) {
  console.error('❌ 请指定版本号或递增类型');
  console.error('   用法: node scripts/bump-version.js <patch|minor|major|x.y.z> [--tag]');
  process.exit(1);
}

const currentVersion = getCurrentVersion();
const newVersion = calcNewVersion(currentVersion, versionArg);

if (currentVersion === newVersion) {
  console.log(`⚠️  版本未变化 (${currentVersion})，无需更新`);
  process.exit(0);
}

console.log(`\n🔖 版本更新: ${currentVersion} → ${newVersion}\n`);

updatePackageJson(newVersion);
updatePackageLockJson(newVersion);
updateCargoToml(newVersion);
updateTauriConf(newVersion);
updateIndexHtml(newVersion);

if (doTag) {
  gitCommitAndTag(newVersion, true);
} else if (args.includes('--commit')) {
  gitCommitAndTag(newVersion, false);
}

console.log(`\n✨ 完成! 所有版本已同步到 ${newVersion}`);
if (!doTag && !args.includes('--commit')) {
  console.log(`   提示: 加 --tag 可自动 git commit + tag`);
}
