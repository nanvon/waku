# Fork 维护规则

本仓库 fork 自 [egoist/waku](https://github.com/egoist/waku)，只供自用。上游会持续更新，
我们要定期合入上游代码，同时保留自己加的功能。所有改动都要以"下次同步时冲突最少、
冲突时能看懂意图"为前提来写。`AGENTS.md` 里的上游规则同样适用。

## 分支

- `upstream` remote 指向 `egoist/waku`，`origin` 指向我们的 fork。
- `main`：只镜像 `upstream/main`，不在上面提交任何东西。
- `local`：我们的工作分支，即 `main` 加上我们的功能提交。
- 本地 `.git/hooks/pre-commit` 会拒绝在 `main` 上提交。hook 不随仓库走，新 clone 要重新放：
  内容是检查 `git symbolic-ref --short HEAD` 为 `main` 时 `exit 1`。
- 多个 agent 并行开发时，每个 agent 用独立的 worktree 或分支，完成后再合回 `local`。
  不要让多个 agent 同时改同一个目录。

## 写代码的规则

1. 原地改，diff 尽量小。不重排、不格式化无关代码、不重命名、不移动上游代码、不顺手重构。
   不对整个文件运行 `cargo fmt`，只格式化自己改动的部分。
2. 逻辑超过十几行时，放进同目录下以 `fork_` 开头的新文件，上游文件里只留一行调用或
   `mod` 声明。
3. 在上游文件里的每处改动都加标记注释 `// [fork] <功能名>`，多行改动在起止处各标一次。
4. 新增内容尽量追加，不要插进上游频繁变动的列表中间（例如按字母排序的 `mod`、
   `actions!`、`bind_keys`、`locales/*.yml` 的 key）。新文案的 key 统一用 `fork.` 前缀。
5. 不新增 `db/migrations` 里的 drizzle 迁移（编号和 `_journal.json` 会和上游冲突）。
   确需持久化时先和维护者确认方案。
6. 尽量不改 `crates/waku-protocol/src/model.rs` 等跨端共享的协议类型。
7. 一个功能一个提交，提交信息以 `fork: ` 开头，写清用途。同一功能的后续修改可以
   `git commit --fixup` 后在同步前合并。
8. 新增或修改功能后，同步更新下方"功能记录"。

## 验证标准

- 每次改完：`cargo check` 通过，并在 dev watcher 重新构建后的 debug app 里确认效果。
- 改到逻辑代码（非纯样式、文案）时：再跑相关模块的测试，例如 `cargo test <测试名>`。
- 同步上游、push 之前：完整测试交给 CI（见下文），CI 通过才算同步完成。

## CI

- 只用 `.github/workflows/fork-ci.yml`：push 到 `local`、提 PR 时在 macOS 上跑 `cargo test --locked`。
  上游没有这个文件，不会冲突。
- 上游的 workflow 在 fork 上用 `gh workflow disable` 停用，不改文件。停用状态保存在 GitHub 上，
  上游更新这些文件后仍然有效。目前 `Tests`（`test.yml`）已停用。
- GitHub 只在某个 workflow 第一次被触发时才注册它，未注册的没法提前停用。`release.yml`（推送 `v*`
  tag 触发）和 `sync-release.yml`（发布 release 触发）至今没被触发过，所以未注册：不要把 `v*` tag
  推到 origin，也不要在 fork 上发布 release。万一触发了，马上停用并取消那次运行。
- 上游新增 workflow 时，它在 fork 上默认是启用的。每次同步后执行
  `gh workflow list -R nanvon/waku --all` 检查，需要时 `gh workflow disable -R nanvon/waku <名称>`。
  `gh` 命令一律带 `-R nanvon/waku`，避免误操作到上游仓库。

## 同步上游

用 rebase，让每个功能始终是 `main` 之上的独立提交：

```bash
git fetch upstream
git tag fork-backup/$(date +%Y%m%d-%H%M) local
git switch main && git merge --ff-only upstream/main && git push origin main
git switch local && git rebase main
```

- 冲突按提交逐个解决。先查"功能记录"里该功能的意图；如果上游重写了相关代码，
  在新代码上重新实现这个意图，不要硬套旧 diff。
- 本地需开启 `git config rerere.enabled true` 和 `git config merge.conflictStyle zdiff3`
  （git 配置不随仓库走，新 clone 要重新设置）。同样的冲突解决过一次会被记住。
- `Cargo.lock` 冲突不手工合并：取上游版本后重新生成。
- 完成后用 `git diff main...local -- . ':!FORK.md'` 检查全部 fork 改动，`cargo check` 通过后
  `git push --force-with-lease origin local`，等 Fork CI 通过，再按上文检查有没有新增 workflow。
- 出问题时回到备份：`git reset --hard fork-backup/<时间>`。

## 功能记录

每个功能一行：用途、涉及的文件、怎么验收。

| 功能 | 用途 | 涉及文件 | 验收方式 |
| --- | --- | --- | --- |
| fork 维护规则 | 本文件，以及 `AGENTS.md` 顶部的引用 | `FORK.md`、`AGENTS.md` | — |
| 关闭官方自动更新 | release 构建不启动 Sparkle，避免官方更新包覆盖本 fork；`WAKU_FORCE_UPDATER=1` 可临时恢复 | `src/updater.rs`（macOS `Updater::init`） | release 包菜单里没有 "Check for Updates"，启动后不发起更新检查 |
| Fork CI | 只在 macOS 上跑测试，替代上游的三平台 CI | `.github/workflows/fork-ci.yml` | push 到 `local` 后 Actions 里只有 Fork CI 在运行，并且通过 |
| 自适应内容宽度 | 对话列和输入框的最大宽度随对话区放宽：对话区减去两侧导航条留白（各 76px），最小 720px、不设上限；窄窗口与上游一致 | `src/app/fork_content_width.rs`；`src/app.rs`（mod、`content_max_width` 字段）、`src/app/render.rs`、`src/app/transcript.rs`、`src/app/transcript_view.rs`、`src/app/composer.rs` 中替换 `CONTENT_MAX_WIDTH` 的位置 | 宽窗口下消息和输入框铺满对话区（两侧各留 76px），左侧导航条仍显示；窄窗口与原来一致 |
