# Guia do usuário — blockexplorer-tui

Este guia descreve, em português, como usar o `blockexplorer-tui`
no dia a dia: o que cada tela mostra, quais atalhos estão
disponíveis e como diagnosticar os erros mais comuns. Para
detalhes de arquitetura e roadmap, veja [`plan/README.md`](../plan/README.md).

## 1. O que é o blockexplorer-tui

`blockexplorer-tui` é um block explorer estilo Etherscan que roda
inteiramente dentro do terminal. Ele fala com provedores externos
(Alchemy para RPC e WebSocket, Etherscan V2 para ABIs e labels,
Sourcify 4byte para assinaturas de funções/eventos) e NUNCA mantém
um índice local — tudo é consultado ao vivo, com cache TTL em
memória quando fizer sentido. A interface é construída em `ratatui`
+ `crossterm`, segue arquitetura hexagonal e é totalmente
navegável apenas por teclado.

## 2. Como rodar

```bash
# Modo demo (sem credenciais): Home com dados congelados
cargo run -- --demo

# Modo ao vivo (Alchemy obrigatório)
ALCHEMY_API_KEY=<sua-chave> cargo run

# Ao vivo com decodificação via Etherscan (recomendado)
ALCHEMY_API_KEY=<alchemy> ETHERSCAN_API_KEY=<etherscan> cargo run

# Escolher outra chain (padrão é Ethereum mainnet)
ALCHEMY_API_KEY=<k> BLOCKEXPLORER_TUI_CHAIN=base cargo run

# Gerar um arquivo de configuração semeado e sair
cargo run -- --init-config
```

Slugs válidos para `BLOCKEXPLORER_TUI_CHAIN`: `ethereum`,
`ethereum-sepolia`, `base`, `polygon`, `optimism`, `arbitrum`. Um
valor inválido é rejeitado com a lista completa no próprio erro.

### Arquivo de configuração opcional

`$XDG_CONFIG_HOME/blockexplorer-tui/config.toml`:

```toml
[credentials]
alchemy = "sua-chave"
etherscan = "sua-chave-etherscan-v2"  # opcional

[defaults]
chain = "ethereum"
```

Variáveis de ambiente sempre têm prioridade sobre o arquivo.

## 3. Atalhos globais

Esses atalhos funcionam em qualquer tela.

| Tecla      | Ação                                               |
|------------|----------------------------------------------------|
| `/`        | Abre a busca universal (modal)                     |
| `?`        | Abre o modal de ajuda com a lista de atalhos       |
| `q`        | Sai do programa                                    |
| `Ctrl+C`   | Sai do programa                                    |
| `Esc`      | Volta uma tela (ou fecha o modal aberto no topo)   |
| `Backspace`| Desativa o cursor de campo sem sair da tela        |

O modal de ajuda lista os atalhos principais descritos abaixo. A
ordem do modal é fixa (testes asserem sobre ela); novas entradas
são apenas acrescentadas ao final.

## 4. Navegação entre telas

A interface organiza telas em uma pilha (`ScreenStack`):

- `Enter` empurra uma tela nova na pilha a partir de seleções,
  valores sob o cursor ou resultados de busca.
- `Esc` sempre faz `pop` e volta para a tela anterior. Um único
  `Esc` remove exatamente uma tela — se você estiver N telas
  abaixo de Home, precisa de N `Esc`. Na Home (pilha de uma só
  tela) o `Esc` é no-op; use `q` ou `Ctrl+C` para fechar o
  programa.
- Para "cancelar" apenas o cursor de campo sem voltar uma tela,
  use `Backspace`.
- O breadcrumb no topo mostra o caminho atual (`Home › Block › Tx`).
- Busca é sempre modal: o overlay fica sobreposto à tela atual, e
  `Enter` numa sugestão empurra a tela correspondente.

## 5. Cursor sobre valores

Cada tela de detalhe tem um cursor que percorre os valores
"navegáveis" (endereços, hashes, números de bloco, ENS, etc.).
Esse cursor é desativado por padrão — ele só "acorda" depois da
primeira seta.

| Tecla            | Ação                                                      |
|------------------|-----------------------------------------------------------|
| `→` / `←` / `↑` / `↓` | Move o cursor entre os campos navegáveis              |
| `y`              | Copia o valor sob o cursor (forma canônica) para o clipboard |
| `Y`              | Copia o identificador canônico da tela (ENS ou hex)       |
| `Enter`          | Abre a tela correspondente ao valor sob o cursor          |
| `Backspace`      | Desativa o cursor (sem sair da tela)                      |

`Esc` é reservado para "voltar uma tela" — um único `Esc` sempre
faz `pop` do topo, independentemente de o cursor estar ativo ou
não. Pressione `Backspace` se você quer apenas desativar o cursor
e continuar na mesma tela.

Quando o cursor está inativo, `y` ainda copia um valor sensato por
tela (hash do bloco, hash da tx, endereço principal, etc.). Todas
as cópias atravessam o adaptador `ArboardClipboard` quando o
ambiente gráfico está disponível; em ambientes headless o adaptador
degrada para um no-op silencioso e o TUI segue vivo.

## 5.5 Rodapé de comandos

A última linha da tela é um "rodapé de comandos" gerenciado pelo
runtime: ele lista os atalhos mais úteis da tela ativa no formato
`[tecla] ação`. Os colchetes e a tecla vêm em negrito, a descrição
fica esmaecida.

- Home → `[/]`, `[?]`, `[s]`, `[q]`.
- AddressDetail → `[Tab]`, `[Arrows]`, `[Enter]`, `[y]`, `[Y]`, `[e]`,
  `[Esc]`. Com o Contract ou Token como aba ativa o rodapé passa
  a anunciar também `[` e `]` para ciclar as sub-abas; na aba
  Chart do Token aparece `[1..3]` para a janela de preço.
- BlockDetail → `[Tab]`, `[`, `]`, `[Arrows]`, `[Enter]`, `[y]`,
  `[Y]`, `[Esc]`.
- TxDetail → `[Tab]`, `[Arrows]`, `[Enter]`, `[y]`, `[s]`, `[Esc]`.
- Settings → `[Arrows]`, `[y]`, `[1..4]`, `[Esc]`.

Quando um overlay modal está aberto (busca `/`, ajuda `?`, etc.), ele
assume a tela inteira; nesse caso o rodapé
do runtime não é desenhado — o próprio overlay desenha suas teclas.

## 6. Tela Home

A Home abre por padrão e mostra:

- Header da chain ativa, status de rede e último bloco.
- Cartão **Gas** com o oráculo resumido (slow / average / fast / base fee).
- Indicador de conexão (Connected / Disconnected / Reconnecting).
- Primeiro-uso: banner que aponta para o Settings quando
  `ALCHEMY_API_KEY` ainda não está configurado.

Atalhos específicos da Home:

| Tecla | Ação                                |
|-------|-------------------------------------|
| `/`   | Abre busca universal                |
| `s`   | Abre Settings                       |

## 7. Busca

O overlay de busca aceita:

- Hashes de transação (`0x` + 64 hex).
- Hashes de bloco (`0x` + 64 hex).
- Números de bloco (decimal).
- Endereços EVM (`0x` + 40 hex).
- Nomes ENS (`*.eth`).
- Tickers de token (quando `ETHERSCAN_API_KEY` está presente).
- URLs de Etherscan (qualquer host conhecido da família Etherscan
  é reconhecido e o caminho é interpretado).

Navegação dentro do overlay:

| Tecla  | Ação                                        |
|--------|---------------------------------------------|
| Digitar | Atualiza a lista de sugestões ao vivo      |
| `↑`/`↓` | Move a seleção                             |
| `Enter` | Empurra a tela do candidato selecionado    |
| `Esc`   | Fecha a busca sem navegar                  |

O overlay desenha o input na base da tela e as sugestões logo
acima. A resolução passa por um cache TTL com chave
`(chain, input)` para não bombardear o RPC com teclas repetidas.

## 8. AddressDetail unificado

Uma única tela (`AddressDetailScreen`) cobre EOAs, contratos e
tokens ERC-20. As abas aparecem dinamicamente de acordo com o
tipo do endereço:

- EOA: `Overview`, `Transactions`, `Tokens`.
- Contrato: as três acima mais `Contract`.
- Token ERC-20: as quatro acima mais `Token`.

Dentro de `Contract` existem 6 sub-abas: Overview, Source, ABI,
Read, Events, Storage. Dentro de `Token` existem 3: Overview,
Transfers, Chart.

| Tecla              | Ação                                                        |
|--------------------|-------------------------------------------------------------|
| `Tab` / `Shift-Tab` | Com foco no corpo ou na faixa de abas principais: próxima / anterior aba principal. Com foco na faixa de **sub-abas**: próxima / anterior sub-aba |
| `←` / `→`          | Só quando o foco está na faixa de abas principais ou de sub-abas: muda aba principal ou sub-aba, respetivamente |
| `↑` / `↓`          | Move o foco entre faixa principal → sub-abas (se existirem) → corpo; no corpo, `↑` no topo de listas (ou no cursor de campos) sobe o foco para as faixas |
| `[` / `]`          | Anterior / próxima sub-aba (em Contract ou Token), em qualquer nível de foco |
| `1` / `2` / `3`    | Janela do gráfico de preço na sub-aba `Token` → `Chart` (`1d` / `1m` / `1y`) |
| `y`                | Copia o endereço em hex (ou o valor sob o cursor)           |
| `Y`                | Copia o nome ENS (ou hex se não houver ENS)                 |
| `e`                | Exporta a aba atual como CSV para o clipboard               |
| `c`                | Pula para a sub-aba Contract quando o token tem metadados inválidos |
| `Enter`            | Abre a tela do valor sob o cursor (ou da linha selecionada) |
| `Backspace`        | Desativa o cursor na Overview (sem sair da tela)            |

Desde abril de 2026, `[` e `]` são a única forma de ciclar as
sub-abas. Os dígitos `1`..`N` ficaram livres para outras funções:
na sub-aba `Token` → `Chart`, `1`/`2`/`3` escolhem a janela de
preço (D1, M1, Y1). Enquanto o editor de argumentos da sub-aba
`Read` está em foco, os dígitos são entregues ao buffer — assim
você consegue digitar argumentos numéricos sem trocar de sub-aba.

## 9. BlockDetail

`BlockDetailScreen` renderiza um bloco completo em três abas:

- `Overview`: número, hash, pai, miner, gas, base fee, timestamp,
  tamanho, extra_data.
- `Transactions`: lista com o hash e a categoria de cada tx.
- `Blobs / Withdrawals`: retiradas do beacon e blobs EIP-4844.

| Tecla       | Ação                                          |
|-------------|-----------------------------------------------|
| `Tab`/`Shift-Tab` | Cicla as abas principais (foco no corpo ou na faixa de abas) |
| `←`/`→`     | Só com foco na faixa de abas: aba anterior / seguinte |
| `↑`/`↓`     | Com foco na faixa: entra no corpo ou sobe do corpo; na aba Transactions move a seleção |
| `[` / `]`   | Bloco anterior / próximo                      |
| `Enter`     | Abre a tx selecionada                         |
| `y`         | Copia hash do bloco (Overview/Blobs) ou hash da tx selecionada (Transactions) |
| `Y`         | Copia o número do bloco                       |

## 10. TxDetail

`TxDetailScreen` renderiza uma transação em 6 abas: `Overview`,
`Logs`, `Internal`, `Asset Changes`, `State Changes`, `Raw`.

- `Overview` mostra From/To, value, gas, status, e os campos são
  navegáveis com o cursor (`Enter` abre a Address/Block correspondente).
- `Logs` decoda eventos usando ABI + openchain + Samczsun, nessa
  ordem. Selecione um log com `↑/↓` e entre em detalhe com `Enter`.
- `Internal` lista a árvore de chamadas (trace), incluindo
  `DELEGATECALL` e `STATICCALL`.
- `Asset Changes` e `State Changes` populam a partir de
  `debug_traceTransaction` e `alchemy_simulateAssetChanges` /
  diff; txs mineradas exibem "unsupported" quando o provedor não
  retorna dados.
- `Raw` mostra o JSON original.

| Tecla                | Ação                                                 |
|----------------------|------------------------------------------------------|
| `Tab` / `Shift-Tab`  | Cicla as abas principais (foco no corpo)             |
| `←` / `→`            | Só com foco na faixa de abas: aba anterior / seguinte |
| `↑`/`↓` ou `j`/`k`   | No corpo: seleção / scroll; `↑` no topo sobe o foco para a faixa de abas; `j`/`k` mantêm ciclo em várias vistas |
| `PageUp`/`PageDown`  | Rolagem por página                                   |
| `y`                  | Copia a linha selecionada (Overview) ou o hash da tx |
| `s`                  | Re-simula a tx (apenas quando pending)               |
| `Enter`              | Abre tela relacionada para o valor sob o cursor      |

Proxies (EIP-1967, UUPS, Transparent) são detectados em cascata:
Alchemy slot-probe primeiro, Etherscan como fallback com cache TTL.
A aba `Overview` mostra o proxy e a implementação via o selo
correspondente.

## 11. Mempool e Gas Tracker (removidos)

A tela de **Mempool** (stream de pendentes) e o **Gas Tracker**
tela cheia foram **abandonados** em abril de 2026; o código e os testes
foram retirados. Os tiers de gas (slow / average / fast / base fee)
continuam só no cartão **Gas** da Home. Ver `plan/5-mempool.md` e
`plan/9-gas-tracker.md` para o registro da decisão.

## 12. Settings

`SettingsScreen` mostra:

- A chain ativa (display name + slug).
- Se a chave Alchemy / Etherscan foi encontrada.
- O caminho do `config.toml` atual (dica, não editável pela tela).
- Um painel de health com o status mais recente dos provedores.
- Presets de paleta numerados: `1` Dark, `2` Light, `3` High
  contrast, `4` Solarized. O preset ativo é marcado com `*`.

O banner de primeiro uso aparece quando não há chave Alchemy e o
binário foi aberto com `--demo`.

## 13. Primeira execução

- Sem chave Alchemy e sem `--demo`: o binário imprime uma dica de
  setup, sugere `--init-config` e sai com código 0 (não é erro).
- Com `--demo`: a Home abre com dados congelados e um banner
  avisa que Settings pode gerar um seed de config.
- `cargo run -- --init-config` cria
  `$XDG_CONFIG_HOME/blockexplorer-tui/config.toml` com um bloco
  `[defaults]` vazio. O arquivo é idempotente: rodar de novo não
  sobrescreve credenciais que já estejam lá.

## 14. Troubleshooting

**`y` não copia nada no terminal.** O adaptador `ArboardClipboard`
tenta abrir uma conexão com o clipboard do sistema no boot. Se
`DISPLAY` (X11) ou `WAYLAND_DISPLAY` não estiverem setados, o
adaptador degrada para um no-op e um `tracing::warn!` é emitido
indicando o motivo. Soluções:

- Rode dentro de uma sessão gráfica (SSH com `-X` funciona para
  X11; sessões `tmux` + `ssh` cruas não têm acesso ao clipboard).
- Em WSL, instale `wl-clipboard` ou `xclip` e exporte `DISPLAY`.
- Em CI / headless: esperado — o TUI continua usável, só o `y` é
  no-op.

**Chain inválida em `BLOCKEXPLORER_TUI_CHAIN`.** A mensagem de
erro lista todos os slugs aceitos (`ethereum`,
`ethereum-sepolia`, `base`, `polygon`, `optimism`, `arbitrum`).
Não há fallback silencioso.

**Links de Etherscan.** O parser de busca aceita URLs dos hosts
`etherscan.io`, `sepolia.etherscan.io`, `basescan.org`,
`polygonscan.com`, `optimistic.etherscan.io` e `arbiscan.io`. O
caminho (`/tx/...`, `/block/...`, `/address/...`) é convertido no
`ResolvedEntity` correspondente.

**"Unsupported" em abas de TxDetail.** O tracer ou o diff não
suportam aquele hardfork / caminho. Isso não é um bug nem uma
falha de rede — o provider retornou `null`. Ao menos a aba
`Overview` continua populada.

## 15. Limitações conhecidas

O MVP deliberadamente não inclui:

- Watchlist persistente.
- Decompilador / verificação de bytecode.
- Suporte a NFTs (coleções, metadados, imagens).
- Aba `Write` (apenas `Read` está exposta).
- Mouse (planejado para um release futuro).

Esses itens estão catalogados em `plan/15-backlog.md` e serão
promovidos para seus próprios arquivos de plano quando priorizados.

---

Para aprofundar: [`plan/README.md`](../plan/README.md) é o índice
mestre. Cada arquivo `plan/N-*.md` documenta uma fatia do
produto, com use cases, portas, BDD scenarios e testes funcionais
em 1:1. Se uma funcionalidade NÃO está descrita em `plan/`, ela
não faz parte do produto — a regra é planning-first.
