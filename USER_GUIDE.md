# Zoom75 TIGA System Monitor — Guida Utente

Monitor in tempo reale per la tastiera Meletrix Zoom75 TIGA.
Legge temperatura CPU, GPU, velocita' ventola e traffico di rete dal Mac
e li mostra sul display integrato della tastiera, ciclando automaticamente
tra le schermate.

## Requisiti

- macOS su Apple Silicon (M1/M2/M3/M4)
- Zoom75 TIGA collegata via USB
- Rust toolchain installata (`rustup`)
- Connessione internet (per geolocalizzazione e meteo)

## Installazione

```bash
cd tiga-fork
cargo build --release --examples
```

I binari compilati si trovano in `target/release/examples/`:
- `tiga_monitor` — monitor in tempo reale con prevenzione sleep
- `tiga_cmd` — comandi singoli per setup, test e ripristino

---

## Guida Rapida

### Scenario 1: Primo collegamento o dopo hard reset

La tastiera appena collegata (o dopo stacco/riattacco USB) non ha orario
ne' meteo. Per impostare tutto in un colpo:

```bash
cargo run --release --example tiga_cmd -- restore
```

Output:
```
Time set to 2026-02-08 12:06:17
Fetching weather...
Weather set (WMO:3, 13/8/14°C)
Restore complete.
```

Il display mostra ora l'orologio aggiornato. Ciclando manualmente dalla
home si vede anche il meteo.

### Scenario 2: Avvio monitoraggio sistema

Per mostrare in tempo reale CPU, GPU, ventola e rete sul display:

```bash
cargo run --release --example tiga_monitor -- -v
```

Output:
```
Sleep prevention active (caffeinate pid=62005).
Connecting to Zoom75 TIGA...
Connected.
Initializing sensors...
Sensors ready (SMC + sysinfo).
Time synced.
Fetching weather... OK (WMO:3, day:true, 14/8/14°C)
Navigated to sysinfo.

Monitor running: interval=2s, cycle=3s, weather=every 30min, verbose
Press Ctrl+C to stop.

[12:57:05] CPU:45°C GPU:44°C Fan:1350RPM Net:0.3Mbps
[12:57:07] CPU:45°C GPU:44°C Fan:1342RPM Net:0.7Mbps
  [screen cycle]
[12:57:09] CPU:45°C GPU:44°C Fan:1355RPM Net:0.4Mbps
```

Il monitor:
1. Impedisce lo sleep del Mac (`caffeinate`) finche' e' attivo
2. Sincronizza ora e meteo
3. Entra nelle schermate sysinfo e cicla tra CPU, GPU, RPM, mbps
4. Aggiorna i valori ogni 2 secondi, cambia schermata ogni 3 secondi
5. Aggiorna il meteo ogni 30 minuti

### Scenario 3: Chiusura pulita

Premi **Ctrl+C**. Il monitor:

```
^C
Shutting down gracefully...
  Navigating to home...
  Syncing time...
  Sending weather (WMO:3, 14/8/14°C)...
Shutdown complete. Display restored to home with time/weather.
Sleep prevention stopped.
```

1. Esce dalle schermate sysinfo
2. Torna alla home (orologio)
3. Ri-sincronizza orario e meteo
4. Disattiva la prevenzione sleep — il Mac puo' dormire di nuovo

### Scenario 4: Solo orario (senza meteo/internet)

```bash
cargo run --release --example tiga_cmd -- time
```

### Scenario 5: Solo meteo

```bash
cargo run --release --example tiga_cmd -- weather
```

### Scenario 6: Display bloccato dopo sospensione

Se il Mac e' andato in sospensione e il display si e' bloccato:

1. Stacca il cavo USB dalla tastiera
2. Ricollega il cavo USB
3. Ripristina orario e meteo:

```bash
cargo run --release --example tiga_cmd -- restore
```

---

## Riferimento Comandi — `tiga_cmd`

Tool per comandi singoli. Ogni invocazione si connette, esegue il
comando e si disconnette.

### Ripristino e sincronizzazione

```bash
# Ripristino completo (orario + meteo) — il piu' utile dopo un reset
cargo run --release --example tiga_cmd -- restore

# Solo orario
cargo run --release --example tiga_cmd -- time

# Solo meteo (richiede internet)
cargo run --release --example tiga_cmd -- weather
```

### Navigazione display

```bash
# Torna alla home (orologio) — usa il reset display, funziona da ovunque
cargo run --release --example tiga_cmd -- home

# Naviga giu' (nel menu: prossimo elemento; in sysinfo: prossima schermata)
cargo run --release --example tiga_cmd -- down

# Conferma selezione / entra nel sottomenu
cargo run --release --example tiga_cmd -- switch

# Torna indietro di un livello
cargo run --release --example tiga_cmd -- return

# Naviga su
cargo run --release --example tiga_cmd -- up
```

### Invio dati sistema manuali

Utile per test e debug. I valori appaiono sulle schermate sysinfo.

```bash
# Formato: sysinfo <CPU°C> <GPU°C> <SSD°C> <FanRPM> <Net>
# Net: il firmware divide per 10, quindi 500 = 50.0 Mbps

# Esempio: CPU=42°C, GPU=77°C, SSD=35°C, Fan=1350 RPM, Net=50.0 Mbps
cargo run --release --example tiga_cmd -- sysinfo 42 77 35 1350 500

# Solo temperature senza fan/net
cargo run --release --example tiga_cmd -- sysinfo 50 60 0 0 0
```

### Navigazione manuale alle schermate sysinfo

Per entrare nelle schermate sysinfo (CPU/GPU/RPM/mbps) manualmente:

```bash
# Dalla home: entra nel menu selezione dati
cargo run --release --example tiga_cmd -- down

# Attiva/evidenzia il menu
cargo run --release --example tiga_cmd -- switch

# Entra nella prima schermata (CPU) — ora vedi il valore numerico
cargo run --release --example tiga_cmd -- switch

# Da qui, "down" cicla tra le schermate:
# CPU → mbps → RPM → GPU → CPU
cargo run --release --example tiga_cmd -- down
```

### Comandi raw (avanzato)

```bash
# Invia un pacchetto con comando e dati arbitrari
# Formato: raw <CMD_HEX> <byte0> <byte1> ...
cargo run --release --example tiga_cmd -- raw FF 0 0 42 0 77 0 0 5 70 0 0
```

---

## Riferimento Opzioni — `tiga_monitor`

### Tutte le opzioni

| Opzione                      | Descrizione                              | Default |
|------------------------------|------------------------------------------|---------|
| `--interval <SEC>`          | Ogni quanti secondi aggiornare i dati    | 2       |
| `--cycle <SEC>`             | Ogni quanti secondi cambiare schermata   | 3       |
| `--no-cycle`                | Resta su una sola schermata              | —       |
| `--no-weather`              | Disabilita sincronizzazione meteo        | —       |
| `--weather-interval <MIN>`  | Ogni quanti minuti aggiornare il meteo   | 30      |
| `-v` / `--verbose`          | Stampa i valori anche a terminale        | off     |
| `-h` / `--help`             | Mostra l'aiuto                           | —       |

### Esempi d'uso

```bash
# Uso standard con output a terminale (consigliato)
cargo run --release --example tiga_monitor -- -v

# Silenzioso (nessun output a terminale, solo il display)
cargo run --release --example tiga_monitor

# Aggiornamento rapido: dati ogni 1s, schermate ogni 2s
cargo run --release --example tiga_monitor -- --interval 1 --cycle 2

# Aggiornamento rilassato: dati ogni 5s, schermate ogni 8s
cargo run --release --example tiga_monitor -- --interval 5 --cycle 8

# Senza meteo (utile offline o senza internet)
cargo run --release --example tiga_monitor -- --no-weather -v

# Meteo aggiornato ogni 15 minuti (invece dei 30 di default)
cargo run --release --example tiga_monitor -- --weather-interval 15

# Schermata fissa (resta sulla schermata corrente, nessun cycling)
cargo run --release --example tiga_monitor -- --no-cycle -v

# Tutto personalizzato: 3s dati, 5s cycling, meteo ogni 10min, verbose
cargo run --release --example tiga_monitor -- --interval 3 --cycle 5 --weather-interval 10 -v
```

---

## Schermate del display

### Home e schermate principali

La home mostra l'orologio. Ciclando manualmente dalla home si accede a
schermate separate: meteo, GIF, animazione, ecc. Non esiste una home
che mostra orario e meteo insieme — sono schermate separate.

### Schermate sysinfo (gestite dal monitor)

Il monitor cicla automaticamente tra 4 schermate dati:

| Schermata | Dato visualizzato                     |
|-----------|---------------------------------------|
| **CPU**   | Temperatura CPU (°C)                  |
| **GPU**   | Temperatura GPU (°C)                  |
| **RPM**   | Velocita' ventola (giri/min)          |
| **mbps**  | Velocita' rete (Megabit/s)            |

L'ordine di cycling e': CPU → mbps → RPM → GPU → CPU

## Come funzionano i sensori

| Sensore    | Fonte                              | Note                              |
|------------|------------------------------------|-----------------------------------|
| CPU temp   | PMU die sensors (hottest cluster)  | Valore piu' alto tra i die        |
| GPU temp   | PMU die sensors (media)            | Media dei restanti die            |
| Fan RPM    | Apple SMC                          | Prima ventola del sistema         |
| Net speed  | Interfacce di rete attive          | Somma upload + download           |

Note:
- Su M4 a idle le ventole sono spente (Fan=0 RPM e' normale)
- I valori CPU/GPU si stabilizzano dopo 2-4 secondi dal primo avvio

## Prevenzione sleep

Il monitor lancia automaticamente `caffeinate -di` all'avvio, che impedisce
al Mac di andare in sospensione (sia display che sistema). Questo e'
necessario perche' se il Mac va in sleep, il collegamento USB si interrompe
e il display della tastiera puo' bloccarsi.

Alla chiusura del monitor (Ctrl+C), caffeinate viene terminato e il Mac
torna al suo comportamento normale.

---

## Risoluzione problemi

**"Zoom75 TIGA not found"**
- Verifica che la tastiera sia collegata via USB (non Bluetooth)
- Verifica che nessun'altra applicazione (es. MeletrixID) stia usando il dispositivo

**"Failed to connect to SMC"**
- Il monitor richiede macOS su Apple Silicon
- Su Intel Mac i sensori temperatura non sono supportati

**Valori CPU/GPU a 0°C**
- Puo' capitare al primissimo avvio; i valori si stabilizzano dopo 2-4 secondi

**Fan sempre a 0 RPM**
- Normale su M4 a idle: le ventole restano spente fino a carico elevato

**Il display non cicla tra le schermate sysinfo**
- Il monitor deve navigare nella gerarchia: home → menu → selezione → CPU
- Se il cycling non funziona, prova a resettare con `tiga_cmd home` e rilanciare

**"Fetching weather... failed"**
- Verifica la connessione internet
- Usa `--no-weather` per avviare il monitor senza meteo
- Il meteo verra' ritentato al prossimo intervallo

**Display bloccato dopo sospensione/sleep del Mac**
- Il monitor impedisce automaticamente lo sleep (`caffeinate -di`)
- Se per qualche motivo il Mac va comunque in sospensione e il display si blocca:
  1. Stacca e ricollega il cavo USB
  2. `cargo run --release --example tiga_cmd -- restore`
