[🇺🇸 Read in English](./README.md)

# RGBStuff

**Controlador unificado de RGB e telemetria de periféricos para Windows 11, ultraleve em Rust + Tauri v2.**

Criado para substituir suítes pesadas e freemium (como o SignalRGB) e unificar os 3 hubs proprietários (**A HUB**, **Aula Hub** e **MCHOSE HUB**) em um daemon nativo em Rust com interface **Windows 11 Mica** que libera totalmente o processo do WebView2 ao fechar — mantendo **0% de CPU em segundo plano** nos modos estáticos e durante jogos.

---

## Principais Recursos

- **Zero WebView2 em Segundo Plano:** Inicia silenciosamente na bandeja do sistema (`windows: []`). Tanto o **Popover da Bandeja** (`360×480`) quanto o **Studio Completo** (`1160×740`) são criados sob demanda com efeito Mica e **destruídos (`window.destroy()`)** ao perder foco ou fechar.
- **Canvas Espacial 2D Híbrido (`rgb-engine`):** Canvas interno `128×72` com amostragem bilinear por LED, limitador de taxa por barramento (`BusFramePacer`: Aura 60 Hz, Aula 30 Hz, RAM 15 Hz) e `DirtyFrameTracker` para pular envios redundantes.
- **Modo Estático Zero-CPU:** Efeitos como `Off`, `Solid` e `Gradient` aplicam uma vez e suspendem o loop do motor via `tokio::sync::Notify` (`0,0%` CPU). O preset padrão **Gaming** usa `Gradient` estático para nunca consumir CPU/GPU enquanto você joga.
- **Ambilight e Áudio Reativo sob Demanda (`rgb-capture`):**
  - **DXGI Desktop Duplication (`64×36`):** Liga **somente** quando o perfil ativo usa `Ambilight` e libera todos os handles DXGI/D3D11 imediatamente ao trocar de perfil.
  - **WASAPI Loopback + `rustfft`:** 16 bandas logarítmicas com janela de Hann, ativo **apenas** no modo `AudioSpectrum`.
- **Regras por App em Foco & Automações (`rgb-automation`):**
  - Troca automaticamente o perfil quando um aplicativo ou jogo entra em foco (regras padrão: `feathertune.exe → ambilight`, `vlc.exe → ambilight`, `cs2.exe → gaming`) e restaura o perfil anterior ao sair.
  - Ignora sobreposições transitórias do Windows e o próprio RGBStuff para que abrir o Popover não quebre a regra ativa.
  - **Seletor rápido de apps abertos** (`EnumWindows`) na aba Automações para adicionar regras em 1 clique.
  - Suporta apagar ao bloquear (`Win+L`), recuperação pós-suspensão, timeout de ociosidade, modo noturno por horário e notificações nativas de bateria baixa.
- **Importador Somente Leitura dos 3 Hubs (`rgb-core::hub_importer`):** Lê configurações e metadados locais de **A HUB (Rapoo)**, **Aula Hub** e **MCHOSE HUB** sem alterar os arquivos originais nem acessar cookies/tokens.
- **Internacionalização Completa (`pt-BR` / `en-US`):** Troca instantânea de idioma salva atomicamente no `config.toml`.

---

## Hardware Suportado

| Componente | Hardware Alvo | VID:PID / Barramento | Recursos (Fase 1) |
| :--- | :--- | :--- | :--- |
| **Placa-mãe & Fans ARGB** | ASUS TUF GAMING A520M-PLUS II (Aura Gen2) | `0B05:19AF` (`USB HID`, `0xFF72`) | Streaming direto per-LED nos 2 headers ARGB (contagem calibrável) |
| **Memória RAM RGB** | Asgard DDR4-3200 (2×8 GB, controlador ENE) | `1022:790B` (`AMD SMBus` via **PawnIO**) | Escrita segura via serviço opcional `rgb-smbus-svc` |
| **Teclado** | Aula F75 HE | `38A6:2729` (`USB HID`, `0xFF68`/`0xFF67`) | Mapeamento no Canvas 2D e detecção ativa (streaming per-key em evolução) |
| **Mouse** | Rapoo VT0 / VT Air Max | `24AE:14A1` (`USB HID`, `0xFF00`) | Leitura passiva de bateria %, carregamento e DPI (Report `0x07`) |
| **Headset** | MCHOSE V9 Pro 2 | `3837:6011` (`USB HID`, `0xFF07`) | Detecção de conexão e telemetria passiva de bateria |

---

## Segurança e Anti-Cheat

- **Sem WinRing0:** Utiliza **PawnIO** (`PawnIOLib.dll`) em vez do driver `WinRing0x64.sys` bloqueado pelo Windows Defender.
- **Mutex Global de SMBus:** Adquire `Global\Access_SMBUS.HTP.Method` antes de transações SMBus para conviver com segurança com HWiNFO, CPU-Z e AIDA64.
- **Proteção Rígida contra Escrita em SPD EEPROM:** `rgb_drivers::validate_smbus_address` permite apenas a whitelist `0x70..=0x77` e bloqueia expressamente `0x50..=0x57` tanto no cliente quanto no serviço.

---

## Como Compilar e Rodar

### Pré-requisitos (Windows 11)
- **Rust** (`1.85+` / Edition 2024) + MSVC Build Tools
- **Bun** (`1.2+`) para o frontend
- **PawnIO** *(opcional, necessário apenas para o RGB da RAM Asgard via SMBus)*

```powershell
# 1. Compilar o frontend
cd apps\desktop
bun install
bunx tsc --noEmit
bun run build

# 2. Rodar todos os testes Rust (na raiz)
cd ..\..
cargo test --workspace

# 3. Gerar executáveis finais em Release
cargo build --release -p rgbstuff-desktop -p rgb-smbus-svc
```

Os executáveis são gerados em:
- `target\release\rgbstuff-desktop.exe`
- `target\release\rgb-smbus-svc.exe`

Consulte [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) para detalhes técnicos da arquitetura e [CONTRIBUTING.md](./CONTRIBUTING.md) para contribuir com novos drivers.

## Licença

[MIT](./LICENSE)
