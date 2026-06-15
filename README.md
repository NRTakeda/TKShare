<div align="center">
  <img src="data/icons/io.github.NRTakeda.TKShare.svg" width="96" alt="TKShare" />
  <h1>TKShare</h1>
  <p><strong>Quick Share nativo para Linux / GNOME</strong></p>

  <p>
    <a href="https://github.com/NRTakeda/TKShare/releases/latest">
      <img alt="Última versão" src="https://img.shields.io/github/v/release/NRTakeda/TKShare?label=download&style=flat-square" />
    </a>
    <img alt="Plataforma" src="https://img.shields.io/badge/Linux-GNOME%20%2F%20Flatpak-blue?style=flat-square" />
    <img alt="Licença" src="https://img.shields.io/badge/licença-GPL--3.0--or--later-green?style=flat-square" />
  </p>
</div>

TKShare envia e recebe arquivos sem fio entre o seu desktop GNOME e dispositivos
Android, usando o protocolo **Quick Share** (Nearby Share) do Google. É um fork
do [Packet](https://github.com/nozwock/packet), construído sobre o motor de
protocolo do [rquickshare](https://github.com/Martichou/rquickshare) (`rqs_lib`),
com foco em desempenho, estabilidade e uma experiência de transferência mais
polida.

## Instalação

A forma recomendada é instalar o pacote pronto a partir do
[último release](https://github.com/NRTakeda/TKShare/releases/latest). Não é
preciso compilar nada.

1. Baixe o arquivo **`TKShare.flatpak`** (seção *Assets* do release).
2. Garanta que o Flatpak e o remote Flathub estão configurados (uma vez só):

   ```bash
   flatpak remote-add --if-not-exists --user flathub https://flathub.org/repo/flathub.flatpakrepo
   ```

3. Instale o pacote:

   ```bash
   flatpak install --user TKShare.flatpak
   ```

4. Abra o **TKShare** pelo menu de aplicativos, ou rode:

   ```bash
   flatpak run io.github.NRTakeda.TKShare
   ```

> Arquitetura suportada no release atual: **x86_64**. O runtime do GNOME é
> baixado automaticamente na primeira instalação caso ainda não esteja presente.

## Recursos

- **Enviar e receber arquivos** com qualquer dispositivo Quick Share / Nearby
  Share por perto, sem app intermediário no celular.
- **Rede temporária assistida (experimental).** Quando os dois dispositivos não
  estão na mesma rede Wi-Fi, o TKShare cria um ponto de acesso temporário direto
  do PC e mostra um **QR code** para o celular se conectar; ao final da
  transferência a rede é desfeita e a conexão anterior é restaurada. Ver
  [Rede temporária](#rede-temporária-assistida).
- **Progresso em círculo com porcentagem**, animado, no estilo do Android.
- **Ícone na bandeja** com ações rápidas: visibilidade, enviar arquivos, abrir
  recebidos e preferências.
- **Arrastar e soltar** para enviar, com realce visual do alvo.

## Por que este fork

Em relação ao Packet original, o TKShare traz:

- **Build otimizado por padrão.** O motor é compilado em modo release mesmo no
  perfil de desenvolvimento. A criptografia (AES) por chunk era dezenas de vezes
  mais lenta em debug, o que limitava muito a velocidade de transferência.
- **Caminho de dados mais enxuto.** Menos cópias de buffer por chunk, sem
  serialização duplicada do corpo cifrado, e `TCP_NODELAY` nos sockets.
- **Mais robustez.** Vários `unwrap()` no handshake e no recebimento viraram
  erros tratados, então um par malformado não derruba mais o app.
- **Descoberta mais confiável.** Reanúncio mDNS periódico para o Android achar o
  PC mais rapidamente.
- **Rede temporária assistida** para transferir mesmo sem uma rede Wi-Fi comum.

## Requisitos

- **Bluetooth ligado** (usado na descoberta do par).
- Para o modo padrão: ambos os dispositivos na **mesma rede Wi-Fi com mDNS**.
- Para a rede temporária: um **adaptador Wi-Fi que suporte modo ponto de acesso**
  (a maioria dos chips Intel/AX recentes suporta) e o `NetworkManager` (`nmcli`).

## Rede temporária assistida

Quando não há uma rede Wi-Fi em comum, o TKShare pode subir um ponto de acesso
temporário no próprio PC. O celular lê o **QR code** exibido no app para se
conectar, a transferência acontece sobre essa rede e, ao final, o hotspot é
desfeito e a sua conexão Wi-Fi anterior é restaurada automaticamente.

> [!IMPORTANT]
> Este recurso é **experimental** e, por segurança, roda em **modo de simulação
> (dry-run) por padrão**: ele mostra a interface e o QR sem mexer na sua conexão
> atual. Para ativar a criação real do ponto de acesso, inicie o app com a
> variável de ambiente `PACKET_HOTSPOT_REAL=1`:
>
> ```bash
> flatpak run --env=PACKET_HOTSPOT_REAL=1 io.github.NRTakeda.TKShare
> ```

## Compilar a partir do código

Necessário apenas para desenvolvimento. O TKShare exige libadwaita 1.8+, por
isso o build é feito via Flatpak.

```bash
# Runtime e extensões (uma vez)
flatpak install flathub org.gnome.Sdk//50 org.gnome.Platform//50 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08 \
  org.freedesktop.Sdk.Extension.llvm20//25.08

# Build de desenvolvimento (app-id .Devel, convive com a versão instalada)
flatpak-builder --force-clean --user --install builddir \
  build-aux/io.github.NRTakeda.TKShare.Devel.json

flatpak run io.github.NRTakeda.TKShare.Devel
```

Para gerar o pacote distribuível (`.flatpak`) usado nos releases, use o
manifesto de produção `build-aux/io.github.NRTakeda.TKShare.json`.

## Créditos

- [Packet](https://github.com/nozwock/packet) por **nozwock** — base da interface
  GTK4 / libadwaita.
- [rquickshare](https://github.com/Martichou/rquickshare) por **Martichou** —
  implementação do protocolo Quick Share (`rqs_lib`).
- Design original por **Dominik Baran**.

Mantido por [Natan Ramos Takeda](https://www.linkedin.com/in/natantakeda).

## Licença

GPL-3.0-or-later, como os projetos dos quais deriva.
