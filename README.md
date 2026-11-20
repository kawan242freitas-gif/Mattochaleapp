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
# Atualizações do Matto Chalés

O launcher consulta a GitHub Release mais recente em `kawan242freitas-gif/Mattochaleapp`. Ao encontrar uma versão superior, troca a placa principal pelo botão **Atualizar**, baixa o instalador e confere o SHA-256 publicado pelo GitHub. O instalador auxiliar aguarda o launcher fechar, mantém uma cópia anterior do executável e reinicia o app sem tocar nos dados em `%LOCALAPPDATA%\Matto Chales\Matto Chales\config`.

## Publicar uma versão

Crie e envie uma tag `vMAJOR.MINOR.PATCH` neste repositório. A Action do GitHub compila o app Windows, cria o instalador e publica a Release com `latest.json`. Exemplo de próxima versão: `v0.1.72`.

O launcher ignora releases da mesma versão ou anteriores. Não publique uma Release sem o instalador `MattoChales-Setup-X.Y.Z.exe` e o digest SHA-256 calculado pelo GitHub.
