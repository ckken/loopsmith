# Loopsmith 发布流程

本文记录 Loopsmith 的构建、产物生成和自动化发布流程。

## 自动化能力

当前仓库包含两个 GitHub Actions workflow：

- `CI`：在 `main` push 和 PR 上运行 `cargo fmt --check`、`cargo clippy --locked --all-targets -- -D warnings`、`cargo test --locked --all-targets`。
- `Release`：在 `v*.*.*` tag 上构建多平台二进制产物，并发布到 GitHub Release。

Release 产物包括：

- `loopsmith-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`
- `loopsmith-vX.Y.Z-x86_64-apple-darwin.tar.gz`
- `loopsmith-vX.Y.Z-aarch64-apple-darwin.tar.gz`
- `loopsmith-vX.Y.Z-x86_64-pc-windows-msvc.zip`
- 每个 archive 对应的 `.sha256`
- 汇总的 `checksums.txt`

## 本地生成当前平台产物

```bash
bash scripts/package-release.sh
```

指定 target：

```bash
bash scripts/package-release.sh aarch64-apple-darwin
```

产物会写入 `dist/`。`dist/` 是本地构建输出，不应提交。

## 发布一个版本

1. 确认版本号：

   ```bash
   sed -n '1,12p' Cargo.toml
   ```

2. 本地验证：

   ```bash
   cargo fmt --check
   cargo clippy --locked --all-targets -- -D warnings
   cargo test --locked --all-targets
   bash scripts/package-release.sh
   ```

3. 提交版本变更并推送：

   ```bash
   git push origin main
   ```

4. 创建并推送 tag：

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

5. GitHub Actions 会自动构建并发布 GitHub Release。

6. 验证 release：

   ```bash
   gh release view v0.2.0 -R ckken/loopsmith
   gh release download v0.2.0 -R ckken/loopsmith -D /tmp/loopsmith-release
   ```

## 安装预编译二进制

私有仓库建议使用 GitHub CLI 下载：

```bash
gh auth login
gh release download v0.2.0 -R ckken/loopsmith -p 'loopsmith-v0.2.0-aarch64-apple-darwin.tar.gz'
tar -xzf loopsmith-v0.2.0-aarch64-apple-darwin.tar.gz
sudo install -m 0755 loopsmith-v0.2.0-aarch64-apple-darwin/loopsmith /usr/local/bin/loopsmith
loopsmith doctor
```

Linux x86_64：

```bash
gh release download v0.2.0 -R ckken/loopsmith -p 'loopsmith-v0.2.0-x86_64-unknown-linux-gnu.tar.gz'
tar -xzf loopsmith-v0.2.0-x86_64-unknown-linux-gnu.tar.gz
sudo install -m 0755 loopsmith-v0.2.0-x86_64-unknown-linux-gnu/loopsmith /usr/local/bin/loopsmith
```

macOS Intel：

```bash
gh release download v0.2.0 -R ckken/loopsmith -p 'loopsmith-v0.2.0-x86_64-apple-darwin.tar.gz'
tar -xzf loopsmith-v0.2.0-x86_64-apple-darwin.tar.gz
sudo install -m 0755 loopsmith-v0.2.0-x86_64-apple-darwin/loopsmith /usr/local/bin/loopsmith
```

Windows x86_64：

```powershell
gh release download v0.2.0 -R ckken/loopsmith -p 'loopsmith-v0.2.0-x86_64-pc-windows-msvc.zip'
Expand-Archive loopsmith-v0.2.0-x86_64-pc-windows-msvc.zip
$env:Path += ";$PWD\loopsmith-v0.2.0-x86_64-pc-windows-msvc"
loopsmith.exe doctor
```

## 从源码安装

```bash
cargo install --git https://github.com/ckken/loopsmith --tag v0.2.0
loopsmith doctor
```

私有仓库需要当前环境有 GitHub 访问权限。

## 基本使用

```bash
loopsmith doctor
loopsmith run --config examples/plaintext-loop.json
loopsmith inspect
loopsmith diff
loopsmith apply --dry-run
```
