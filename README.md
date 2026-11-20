# Matto Chalés para Windows (Rust nativo)

Aplicativo desktop independente, com interface implementada em Rust/egui. A janela não carrega a interface do site. As APIs HTTPS da produção sincronizam produtos, pedidos, reservas e dados financeiros.

## Dados do site

Reservas, movimentações e contas a pagar mantêm um cache local e sincronizam com o MongoDB do site pela API protegida da Vercel. Para conectar, abra **Configurações → Dados e conexão** e informe a chave administrativa configurada como `ADMIN_API_TOKEN` no projeto Vercel. A chave fica apenas em memória durante a sessão. Sem conexão, o app continua usando o cache local.

O backend correspondente está no repositório `Mattochal-sadega`, no arquivo `api/business-data.ts`. O deploy desse backend deve incluir esse endpoint antes que a sincronização remota fique disponível.

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

Crie e envie uma tag `vMAJOR.MINOR.PATCH` neste repositório. A Action do GitHub compila o app Windows, cria o instalador e publica a Release com `latest.json`. Exemplo de próxima versão: `v0.1.78`.

O launcher ignora releases da mesma versão ou anteriores. Não publique uma Release sem o instalador `MattoChales-Setup-X.Y.Z.exe` e o digest SHA-256 calculado pelo GitHub.
