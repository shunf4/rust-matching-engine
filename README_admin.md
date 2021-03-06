﻿本后端采用 Rust actix-web + Diesel 编写，需要使用 PostgreSQL 作为数据库后端。

请首先安装 PostgreSQL，并启用其服务。Windows 需要在 Windows
服务里打开。

请安装 PostgreSQL 的开发库，确保系统中有 libpq 这个库。

在 Windows 下，请先确保 libpq 路径环境变量 PQ_LIB_DIR（路径
例子：PQ_LIB_DIR=C:\Program Files\PostgreSQL\12\lib） 正确
且 PATH 里有包含 libpq.dll（仅限 Windows，如C:\Program Fil
es\PostgreSQL\12\bin。不要通过运行 C:\Program Files\Postgr
eSQL\12\pg.env.bat 的方式来添加，这会带来额外的引号）的目
录。

在上述条件具备后，运行

```
cargo build
cargo install diesel_cli --no-default-features --features postgres
```

构建主程序（如果使用已经编译好的二进制程序 `rust-matching-engine`，
可以不 `cargo build`），并安装 diesel 管理程序。

请安装 PostgreSQL，为本应用程序建立专用的数据库用户、密码和数据库，
并将 `postgresql://` 协议的数据库地址替换到本路径下的 `.env` 隐藏
文件中。diesel 管理程序、本后端都会读取 `.env` 文件的内容，或环境
变量中 `DATABASE_URL` 变量的内容作为数据库链接地址。

如果想要修改后端绑定到的 IP 地址和端口号，也可以在 `.env` 中修改或
增加环境变量 `LISTEN_HOST_PORT`。

之后执行
```
diesel migration run
```
导入数据库模式。

执行
```
cargo test -- --nocapture
```
看是否能正常插入和读取用户。

执行
```
diesel migration redo
```
清空数据库，之后再执行
```
cargo run
```
或者（如果使用直接编译好的二进制程序）
```
./rust-matching-engine
```
就可以运行后端程序了。
