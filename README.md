<div align="center">
  <img src="data/icons/io.github.NRTakeda.TKShare.svg" width="96" />
  <h1>TKShare</h1>
  <p><strong>Quick Share nativo para Linux / GNOME</strong></p>
</div>

TKShare envia e recebe arquivos sem fio com dispositivos Android usando o
**Quick Share** (Nearby Share) do Google, direto do seu desktop GNOME. É um
fork do [Packet](https://github.com/nozwock/packet), construído sobre o motor
de protocolo do [rquickshare](https://github.com/Martichou/rquickshare)
(`rqs_lib`).

## Por que este fork

TKShare nasceu de uso pessoal e foca em desempenho, estabilidade e uma
experiência de transferência mais polida. Em relação ao Packet original:

- **Build otimizado por padrão.** O motor é compilado em modo release mesmo no
  perfil de desenvolvimento. A criptografia (AES) por chunk era dezenas de
  vezes mais lenta em debug, o que limitava muito a velocidade de transferência.
- **Caminho de dados mais enxuto.** Menos cópias de buffer por chunk, sem
  serialização duplicada do corpo cifrado, e `TCP_NODELAY` nos sockets.
- **Progresso em círculo com porcentagem**, animado, no estilo do Android, em
  vez de uma barra com tempo restante.
- **Mais robustez.** Vários `unwrap()` no handshake e no recebimento viraram
  erros tratados, então um par malformado não derruba mais o app.
- **Descoberta mais confiável.** Reanúncio mDNS periódico para o Android achar
  o PC mais rápido.
- **Tray com ações rápidas** (visibilidade, enviar, abrir recebidos,
  preferências) e animações de transferência.

## Requisitos

Apenas o meio Wi-Fi LAN é implementado, então o TKShare precisa de **Bluetooth
ligado** e de ambos os dispositivos na **mesma rede Wi-Fi com mDNS**.

## Build

Requer a runtime GNOME via Flatpak:

```bash
flatpak install flathub org.gnome.Sdk//50 org.gnome.Platform//50 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08 \
  org.freedesktop.Sdk.Extension.llvm20//25.08

flatpak-builder --force-clean --user --install builddir \
  build-aux/io.github.NRTakeda.TKShare.Devel.json

flatpak run io.github.NRTakeda.TKShare.Devel
```

## Créditos

- [Packet](https://github.com/nozwock/packet) por **nozwock** — base da interface
  GTK4/libadwaita.
- [rquickshare](https://github.com/Martichou/rquickshare) por **Martichou** —
  implementação do protocolo Quick Share (`rqs_lib`).
- Design original por **Dominik Baran**.

## Licença

GPL-3.0-or-later, como os projetos dos quais deriva.
