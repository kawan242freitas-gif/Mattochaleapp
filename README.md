# Matto Chalés para Windows (Rust nativo)

Aplicativo desktop independente, com interface implementada em Rust/egui. A janela não carrega a interface do site. As APIs HTTPS da produção são usadas somente para sincronizar produtos e pedidos.

## Executar

```powershell
cargo run
```

## Compilar

```powershell
cargo build --release
```

O executável fica em `target/release/matto-chales-desktop.exe`.
