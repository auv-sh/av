# av

An extremely handy AV search and downloader, written in Rust.

受 [astral-sh/uv](https://github.com/astral-sh/uv) 的 README 风格启发。

## Highlights

- 🚀 单一工具覆盖“搜索/详情/列表/下载”全流程
- ⚡️ 异步抓取，尽量快速返回结果（JavDB 优先，Sukebei 兜底并合并磁力）
- 🧾 `--json` 输出，易于脚本与二次处理
- 🧲 下载优选做种数最高的磁力链接
- 🖥️ 跨平台（macOS / Linux / Windows），无强制下载依赖（可选 `aria2c`）

## Installation

Build from source（需要 Rust 稳定版工具链）：

```bash
git clone <your-repo-url> av && cd av
cargo build --release
./target/release/av --help
```

可选：安装到 PATH

```bash
sudo cp target/release/av /usr/local/bin/
```

下载相关的可选依赖：

- 推荐安装 `aria2c` 以获得更可控的下载体验
  - macOS: `brew install aria2`
  - Linux/Windows: 请使用对应包管理器安装
- 若未安装 `aria2c`，将自动调用系统默认的磁力处理程序（macOS: `open` / Linux: `xdg-open` / Windows: `start`）

## Quickstart

```bash
# 搜索（演员或番号），默认表格输出
av search 三上悠亞
av search FSDSS-351 --json

# 查看详情（含更多字段）
av detail FSDSS-351

# 列出演员的所有番号（表格 + 总数）
av list 橋本ありな

# 下载（install 的别名：get）
av get FSDSS-351
```

## Features

### Search

```bash
av search <keyword> [--json]
```

- 支持演员名与番号两类查询
- 非 JSON 模式使用表格展示：`# / 番号 / 标题`，顶部显示“共 N”

### Detail

```bash
av detail <code> [--json]
```

展示（可用时尽量完整）：

- 番号、标题、演员、发行日期、封面
- 剧情、时长、导演、片商、厂牌、系列、类别标签、评分
- 预览图列表
- 磁力链接总数与前几条示例

### List

```bash
av list <actor> [--json]
```

- 列出演员的所有番号，表格展示并显示总数

### Install / Get

```bash
av install <code>
av get <code>        # install 的别名
```

- 自动抓取并选择做种数更高的磁力链接
- 优先使用 `aria2c` 下载；缺失时交给系统默认 BT 客户端

## Output

- 所有子命令均支持 `--json` 输出，适合管道与脚本
- 非 JSON 模式专注可读性：
  - `search` / `list`：表格 + 总数
  - `detail`：字段分组展示

## Data sources

- 详情与搜索：JavDB（优先）
- 磁力与兜底：Sukebei（必要时合并磁力详情）

注意：字段可用性取决于页面结构与可见性，可能受地区、反爬或镜像差异影响。

## Platform support

已在 macOS / Linux 下验证构建与运行；Windows 需使用等价命令行环境。

## Acknowledgements

- README 组织形式参考了 [astral-sh/uv](https://github.com/astral-sh/uv)

## License / Disclaimer

本工具仅用于学习与技术研究，使用产生的风险由使用者自行承担。请在遵守当地法律法规与站点条款前提下使用。
