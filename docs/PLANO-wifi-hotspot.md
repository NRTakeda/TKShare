# Plano: transferência sem rede comum via Wi-Fi Hotspot

Objetivo: permitir que o TKShare envie/receba arquivos quando o PC e o celular
**não estão na mesma rede Wi-Fi**, criando um hotspot temporário, como o Quick
Share oficial faz.

## Por que hotspot (e não Bluetooth)

- O Android, ao falar com clientes terceiros, negocia transporte **Wi-Fi**.
  Não aceita transferência por Bluetooth puro de fora do app oficial.
- BLE é lento demais (~1-2 KB/s) e o motor só o usa para descoberta.
- O hardware deste PC (Intel AX201) suporta modo AP (`nmcli`:
  `WIFI-PROPERTIES.AP: sim`), então criar hotspot é viável.

## Como o Quick Share faz (resumo do protocolo)

1. Descoberta acontece (BLE + mDNS), conexão inicial sobe por algum meio.
2. Um lado oferece um **bandwidth upgrade** listando meios suportados
   (`UpgradePathInfo` com `Medium`).
3. Para `WIFI_HOTSPOT`: o lado que vai hospedar cria um AP e envia
   `WifiHotspotCredentials` (ssid, password, gateway, frequency, port).
4. O outro lado conecta nesse AP e abre o socket TCP lá.
5. Transferência ocorre; ao final, o hotspot é desfeito e a rede anterior
   restaurada.

## Estado atual do código (o que já existe)

- `outbound.rs:248` anuncia **apenas** `mediums: vec![Medium::WifiLan]`.
- O `.proto` já define `WifiHotspotCredentials` e o enum `Medium::WifiHotspot`.
- O motor já sabe **parsear** credenciais de Wi-Fi (`parse_password_payload`
  em inbound.rs) — usado hoje para receber compartilhamento de senha de Wi-Fi.
- `bandwidth_upgrade_negotiation_frame` está disponível como tipo.

## O que falta implementar

### A. Camada de hotspot do SO (novo módulo, ex. `hdl/hotspot.rs`)
- `create_hotspot()`: via `nmcli device wifi hotspot ...` (ou D-Bus do
  NetworkManager), gerando SSID/senha aleatórios. Retorna credenciais + IP do
  gateway.
- `connect_to_hotspot(creds)`: via `nmcli device wifi connect ...`.
- `teardown()`: derruba o hotspot/conexão e **restaura a conexão Wi-Fi
  anterior** (guardar o nome da conexão ativa antes).
- Tratar permissões: no Flatpak, `nmcli` precisa de acesso ao D-Bus do
  NetworkManager (`--talk-name=org.freedesktop.NetworkManager` no manifesto) e
  provavelmente polkit para criar AP.

### B. Negociação de meio (no handshake)
- Oferecer `Medium::WifiHotspot` além de `WifiLan` quando habilitado.
- Implementar o frame de `BANDWIDTH_UPGRADE_NEGOTIATION`:
  enviar/receber `UpgradePathInfo` e escolher o meio.
- Decidir papéis: quem hospeda o AP (geralmente quem recebe) e quem conecta.

### C. Orquestração na transferência
- Se cair em hotspot: host cria AP -> envia creds -> espera o peer conectar ->
  reabre o listener/conexão no IP do AP -> segue o fluxo normal de envio.
- Timeout e rollback se o peer não conectar (derrubar AP, restaurar rede).

### D. UI / preferências
- Opção "Permitir hotspot quando não houver rede comum".
- Feedback no app: "Criando rede temporária...", "Conecte o outro
  dispositivo...", etc.

## Riscos e cuidados (honestos)

- **Mexe na conexão de rede ativa do PC** durante o desenvolvimento/teste —
  pode te desconectar da internet temporariamente. Testar com cuidado.
- Sandbox do Flatpak + NetworkManager + polkit: pode exigir permissões que nem
  sempre são concedidas; talvez precise de fallback ou aviso ao usuário.
- Compatibilidade real com o Android só se confirma testando com o aparelho —
  o protocolo de upgrade tem detalhes (ordem de frames, formato exato das
  credenciais) que a documentação não cobre 100%.
- Esforço estimado: alto (vários dias), melhor feito em fases testáveis.

## Fases sugeridas (incrementais e testáveis)

1. **Módulo hotspot isolado** + um pequeno teste manual (criar/derrubar AP via
   nosso código, sem tocar no protocolo). Valida a parte de SO sem risco ao
   handshake.
2. **Negociação de meio** sem hotspot real ainda (só trocar os frames e logar a
   escolha), validando o handshake estendido.
3. **Integrar**: host cria AP, peer conecta, transferência real. Testar
   PC -> Android.
4. **UI + rollback robusto** (restaurar rede, timeouts, mensagens).

Cada fase é um build/commit separado, para não quebrar o que já funciona.
