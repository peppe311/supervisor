# Codex App Server: prossimi passi, piano di rilascio e audit finale

Aggiornamento successivo **P3-A, 2026-09-09**: completati nel backend metadati Git,
sezioni native e revert protetto; `thread/items/list` preparato ma bloccato nel
trasporto. Il test isolato con App Server ufficiale verifica anche lo stato
finale e la riconciliazione dopo riavvio simulato. Nessuna nuova UI o build di
consegna: l'audit Astra/visuale sotto resta quello della baseline P2, non una
certificazione delle nuove funzioni P3-A. Dettagli e limiti in
[CODEX_APP_SERVER_P3A.md](CODEX_APP_SERVER_P3A.md).

Data dell'audit: **2026-09-09**  
Repository: `C:\Users\<user>\Documents\Central Agent`  
Commit di base osservato: `3c2f1273e051c8da5a27ba19c4f10183efe2d9cd`  
Protocollo adottato: **Codex App Server stabile 0.153.4**, con API sperimentali disabilitate  
Revisore indipendente: **GPT-6 Astra**, reasoning `xhigh`  
Controllo visuale: eseguito tramite Computer Use sulla build debug precedente al
follow-up di persistenza, che non modifica la presentazione

## Stato esecutivo

Il progetto dispone nel sorgente delle funzionalita Codex App Server selezionate
per P0, P1 e P2. I quattro finding P2 originariamente rilevati dall'audit Astra,
e il successivo finding P2 sulla persistenza della receipt paginata, sono stati
corretti, coperti da regressioni e sottoposti al gate completo. Il
micro-fix Apps 401/403 e stato confermato anche visivamente nei temi Dark e
Light.

Lo stato corrente e:

- **P0:** chiuso nell'ambito stabile selezionato;
- **P1:** chiuso nell'ambito stabile selezionato;
- **P2:** chiuso nel sorgente e nei controlli autonomi; i quattro finding
  originari e il finding di follow-up sulla persistenza sono risolti;
- **gate completo:** superato il 2026-09-09;
- **release tecnica locale:** **GO** come anteprima non firmata e non
  distribuibile;
- **release pubblica/enterprise:** **NO-GO** fino a firma, registrazione client,
  revisione del commit/licenze e prove che richiedono account, OAuth, UAC o
  side effect reali;
- **Apps 401/403:** micro-fix confermato;
- **anteprima corrente:** `outputs/codex-p2-source-preview-2026-09-09`, costruita
  dal worktree corrente e marcata esplicitamente come non distribuibile.

## Aggiornamento di chiusura autonoma — 2026-09-09

Sono state completate tutte le attivita che non richiedono una decisione o una
verifica personale:

- l'import resta busy fino alla completion, rifiuta sovrapposizioni e diventa
  incerto dopo una disconnessione post-ACK;
- terminate distingue rifiuto certo, risposta malformata, consegna incerta e
  completion concorrente, senza lasciare controlli incoerenti;
- feedback, reset ed email terminano sempre in uno stato esplicito; la chiave
  idempotente del reset sopravvive soltanto quando l'esito e incerto;
- `settings_revision` impedisce a risposte tardive di start/resume/fork di
  sovrascrivere una notification piu recente o di rendere nuovamente corrente
  uno snapshot invalidato;
- preferenze, skill e Goal serializzano directory Windows di presentazione senza
  prefissi estesi, mantenendo l'identita canonica Rust;
- l'idratazione paginata riconcilia una ricevuta incerta soltanto sullo stesso
  `clientId` nativo e salva il binding prima di pubblicare `history_hydrated`;
  lo shutdown chiude stdin e concede al processo posseduto il tempo di scaricare
  la rollout prima del kill di sicurezza;
- l'oracolo Goal distingue correttamente gli eventi live dalla persistenza
  differita di App Server, mentre una prova separata di riavvio verifica la
  persistenza effettiva;
- tutte le 19 probe runtime opt-in sono state eseguite: 15 passano. Le altre
  quattro documentano limiti del runtime 0.153.4 e non vengono emulate dal
  client: nessuna notification MCP progress, nessuna callback network-policy,
  profilo nominato disponibile solo con API sperimentale e review print-only
  bloccata dalla policy prima della callback di approvazione;
- i quattro percorsi fisici di perdita del wire, main/graph e prima/dopo ACK,
  passano con recupero della ricevuta esatta, nessun replay e continuazione
  dello stesso thread;
- i percorsi no-inference di settings, MCP options/configuration, draft,
  elicitation, comando, Goal e configurazione/reload sono passati in main e
  graph dove applicabile;
- `scripts/verify-workspace.ps1` passa con 693 test Rust ordinari, 22 test
  ignorati dal gate workspace (incluse 19 probe runtime), tre scope-probe, 171
  test frontend passati e una
  cattura immagine intenzionalmente skipped;
- `cargo build --locked --release --workspace`, lo smoke test hidden-WebView,
  la verifica degli eseguibili non firmati e il manifest a cinque record sono
  passati.

Restano locali sei profili sintetici `.codex-*`, circa 11,7 MiB in totale. Sono
stati risolti e classificati come artefatti di audit non tracciati, ma la policy
di esecuzione ha negato la loro rimozione esatta. Il blocco non e stato aggirato
con un'altra shell o API.

## Ordine operativo raccomandato

### 1. Congelare temporaneamente le nuove feature — completato

Non introdurre marketplace, plugin, nuovi metodi `fs/*` o API sperimentali prima della chiusura della release corrente. Il lavoro immediato deve riguardare esclusivamente regressioni, prove, packaging e documentazione.

Condizione di uscita:

- nessun cambiamento funzionale estraneo ai cinque finding P2 complessivi e ai
  gate di rilascio;
- worktree e artefatti di prova classificati prima del commit finale;
- nessuna cancellazione automatica delle directory `.codex-*` finche proprieta e recuperabilita non sono state accertate.

### 2. Correggere i quattro finding P2 originari — completato

Ordine suggerito:

1. mantenere l'importazione asincrona occupata fino alla notification di completamento;
2. recuperare uno stato controllabile quando `command/exec/terminate` viene rifiutato;
3. rendere terminali e non contraddittori gli stati di feedback, reset ed email;
4. aggiungere una revisione alle impostazioni native per impedire l'overwrite da una risposta tardiva.

Per ogni correzione servono test su entrambi gli ordini possibili degli eventi, inclusi ACK tardivi, notification anticipate, disconnessione e completion concorrente.

Condizione di uscita:

- i quattro finding originari non sono piu riproducibili staticamente o tramite
  rendering SSR;
- ogni operazione possiede un esito esplicito: completata, fallita, rifiutata, annullata oppure incerta;
- nessuna mutazione viene ritentata automaticamente dopo un esito incerto;
- una disconnessione non trasforma uno stato vecchio in uno stato apparentemente corrente.

### 3. Aggiornare i test di regressione — completato

Aggiungere almeno le seguenti coperture:

- import: richiesta, ACK, progress, completion e disconnessione dopo ACK;
- import: tentativo di seconda mutazione mentre la prima e ancora attiva;
- terminate: rifiuto certo, risposta non valida, consegna incerta e completion arrivata prima della risposta;
- feedback/reset/email: errore RPC e risposta malformata senza testo `uploading` o `requesting` residuo;
- reset: conservazione dell'identita del tentativo quando l'esito puo essere incerto;
- settings: notification aggiornata prima dell'ACK e ACK prima della notification;
- settings: `notLoaded` o disconnect prima di una risposta tardiva;
- rendering Svelte dei nuovi stati terminali in tema Dark e Light.

### 4. Rigenerare il frontend ed eseguire il gate completo — completato

Dopo le correzioni:

1. eseguire `scripts/build-frontend.ps1`;
2. non modificare manualmente `ui/dist`;
3. eseguire `scripts/verify-workspace.ps1`;
4. eseguire `cargo build --locked` e la build release destinata al packaging;
5. verificare `git diff --check` e `cargo fmt --all -- --check`.

Condizione di uscita:

- frontend compilato dal sorgente corrente;
- bundle embedded corrispondente al sorgente Svelte/TypeScript;
- nessun test fallito, errore Svelte, warning Clippy bloccante o violazione dei confini provider;
- advisory delle dipendenze esplicitamente classificati nel documento di sicurezza.

### 5. Ripetere l'audit visuale delle superfici interessate — completato nei limiti autonomi

Il micro-fix Apps e gia stato osservato correttamente il 2026-09-09. Dopo le correzioni P2 va comunque ripetuto un audit visuale mirato su:

- import in attesa, in progresso, completato, fallito e incerto;
- comando running, stopping, terminate rifiutato, completato e connection closed;
- feedback/reset/email in corso, completati, falliti e incerti;
- impostazioni native correnti e stale dopo resume/disconnect;
- Apps non disponibili, catalogo vuoto realmente corrente e catalogo popolato;
- reconnessione e rimozione dei soli alert locali ormai superati;
- superfici principali e graph agent nei temi Dark e Light;
- profili Full HD, 2K e 4K previsti dal contratto di design.

Il controllo deve verificare testo, gerarchia, affordance abilitate/disabilitate, scroll, focus da tastiera, assenza di sovrapposizioni e distinzione tra stato corrente, stale, errore e risultato vuoto.

### 6. Eseguire le validazioni manuali bloccanti

Le prove seguenti richiedono una persona, account o ambiente reali e non possono essere certificate completamente da Computer Use:

#### Autenticazione e account

- login e logout ChatGPT;
- device code;
- sessione scaduta e successivo recupero;
- refresh dell'account;
- account/workspace con Apps effettivamente abilitate;
- revoca e scadenza OAuth;
- reset di un credito reale, solo quando intenzionalmente disponibile;
- invio reale di workspace email e feedback testuale.

#### Apps e MCP

- catalogo Apps positivo con almeno un connettore autorizzato;
- inserimento deliberato di una app mention nel prossimo input;
- OAuth, scope, revoca e scadenza di un connettore;
- MCP OAuth reale;
- tool MCP read-only e side-effecting con approvazioni reali;
- form `openai/form` reali e relativi esiti;
- reload MCP e callback/modalita effettivamente esposte dal runtime.

#### Sicurezza Windows e comandi

- UAC ed elevazione;
- sandbox Windows disponibile, non disponibile, legacy e riavvio richiesto;
- confini reali di filesystem e rete per ogni profilo di accesso;
- comando App Server con stdin, resize, terminate e completion;
- rifiuto della terminazione e recupero dei controlli dopo il fix;
- verifica che argv non diventi una shell interpolata;
- timeout e chiusura della connessione durante un processo.

#### Handoff e lifecycle

- picker Windows reale;
- allegati testuali, PNG/JPEG e audio reali;
- riavvio dopo modifiche di configurazione;
- importazione da un agente esterno;
- Goal, Review e Compact con lavoro reale del modello;
- autorizzazioni Allow once, Allow for session, Decline e Cancel;
- eliminazione nativa permanente esclusivamente su conversazioni di prova.

La checklist operativa dettagliata rimane in [CODEX_APP_SERVER_USER_VALIDATION.md](CODEX_APP_SERVER_USER_VALIDATION.md).

Condizione di uscita:

- ogni prova ha data, ambiente, account di test, risultato ed evidenza;
- nessun dato personale o conversazione importante viene usato per i test distruttivi;
- ogni failure e classificata come difetto Central Agent, limitazione account/workspace o comportamento upstream.

### 7. Correggere gli eventuali problemi emersi e ripetere i gate

Qualsiasi problema trovato nelle prove manuali deve essere corretto prima del packaging. Dopo ogni correzione materiale vanno ripetuti almeno il test specifico, il gate completo e il controllo visuale della superficie interessata.

### 8. Produrre un nuovo pacchetto release — anteprima locale completata

Non distribuire `outputs/codex-p1-final/CentralAgent.exe` come build corrente:
precede P2 e gli ultimi hardening. L'anteprima P2 corrente e stata prodotta, ma
non soddisfa i requisiti di una release pubblica perche nasce da modifiche non
committate ed e priva di firma Authenticode.

Il nuovo pacchetto deve:

- essere generato dal commit che ha superato tutti i gate;
- includere il bundle frontend rigenerato;
- riportare versione, data, commit e SHA-256;
- mantenere `experimentalApi: false`;
- includere licenze, notice e documentazione aggiornata;
- essere firmato e registrato secondo il processo Windows/enterprise previsto, se applicabile.

### 9. Eseguire uno smoke test da profilo pulito — parte automatica completata

Usare un profilo temporaneo senza stato precedente e verificare:

- primo avvio;
- connessione App Server;
- caricamento cronologia;
- resume;
- richieste di approvazione;
- Apps unavailable e, con account idoneo, catalogo positivo;
- riavvio e persistenza;
- tema Dark e Light;
- chiusura pulita senza processi orfani.

Il profilo temporaneo va rimosso soltanto dopo avere risolto il percorso esatto e verificato che non contenga dati da conservare.

### 10. Chiudere documentazione e release

Aggiornare:

- [CODEX_APP_SERVER.md](CODEX_APP_SERVER.md);
- [CODEX_APP_SERVER_ACCEPTANCE.md](CODEX_APP_SERVER_ACCEPTANCE.md);
- [CODEX_APP_SERVER_GAP_ANALYSIS.md](CODEX_APP_SERVER_GAP_ANALYSIS.md);
- [CODEX_APP_SERVER_PLAN.md](CODEX_APP_SERVER_PLAN.md);
- [CODEX_APP_SERVER_USER_VALIDATION.md](CODEX_APP_SERVER_USER_VALIDATION.md);
- changelog, versione, checksum e limitazioni note.

Solo dopo il completamento dei gate va creato il commit/tag di release.

### 11. Selezionare il prossimo blocco funzionale

Dopo la release stabile, l'ordine consigliato e:

1. **P3: marketplace e gestione plugin**;
2. metodi thread avanzati, inclusi listing degli item, sezioni, revert e metadati Git dove utili;
3. eventuale bridge `fs/*`, soltanto se non crea una seconda autorita sul filesystem;
4. API sperimentali realtime, ambienti remoti e collaborazione, per ultime e dietro una decisione esplicita di prodotto.

## Artefatti correnti

| Artefatto | Stato | Dimensione | SHA-256 |
| --- | --- | ---: | --- |
| `target/debug/central-agent.exe` | Build debug corrente dopo il gate | 66.541.056 byte | `7b6bf8853a03c27929b2da8d8ecbd8e66775b42b34862f038399044761dd078e` |
| `outputs/codex-p1-final/CentralAgent.exe` | Vecchio pacchetto P1; non contiene P2 e tutti gli hardening correnti | 33.646.080 byte | `0ebb246ff6a1a05b9e1907dfc9780fea38a6b4f38f11216e0523b312d80cc6a7` |
| `outputs/codex-p2-source-preview-2026-09-09/CentralAgent.exe` | Anteprima P2 ottimizzata, non firmata e non distribuibile | 34.142.720 byte | `8eb9dce7449eabace427d4c30a1cc2626fd0003a7fb09ed6b7eb474adda521b2` |
| `outputs/codex-p2-source-preview-2026-09-09/CentralAgentMcp.exe` | MCP companion della stessa anteprima | 5.645.824 byte | `c355cc9757226c92bd1aa1a5a7c5f69951f852b27ea6813ac895a167b070edce` |

Il manifest schema v2, l'inventario delle 690 dipendenze, licenza e notice sono
stati verificati sul disco. Il pacchetto e deliberatamente etichettato
`unsigned-development`, `dirty-uncommitted` e `distributable: false`.

---

# Audit indipendente finale GPT-6 Astra

## Mandato e metodo

GPT-6 Astra ha svolto una revisione indipendente, senza modificare sorgenti, test o bundle, dei seguenti elementi:

- client Rust del protocollo stabile 0.153.4;
- proiezioni e mirror di thread, turn, settings, notice e usage;
- ownership e authority boundary tra Rust, IPC e Svelte;
- lifecycle di connessione, richieste, history, resume, fork, review e goal;
- Apps, MCP, hooks, skills e workflow P2;
- stati finali, errori, race, disconnect e retry;
- test Rust, frontend, scope probe, contract verifier e gate statici;
- documentazione e dichiarazioni di copertura.

L'audit Astra e stato affiancato da un controllo visuale live separato, eseguito dal processo principale tramite Computer Use sull'eseguibile debug corrente. Le due fonti sono tenute distinte: Astra non ha dichiarato di avere svolto personalmente l'ispezione desktop.

## Verdetto dell'audit

Il verdetto originale Astra ha trovato quattro finding P2 confermati e nessun
nuovo difetto P0/P1. Quel risultato resta qui come traccia indipendente. La
remediation successiva li ha chiusi tutti e quattro con regressioni dedicate,
verifica del bundle e gate completo verde.

Il sorgente corrente e quindi **GO tecnico per P0/P1/P2 stabile selezionato**.
Non e ancora una release pubblica: account, OAuth, UAC, sandbox reali, firma e
registrazione enterprise conservano autorita esterna o richiedono una decisione
umana.

Le descrizioni tecniche nelle sezioni Finding 1-4 conservano intenzionalmente lo
stato pre-remediation esaminato da Astra; i paragrafi `Stato 2026-09-09` e il
follow-up finale hanno precedenza sul tempo verbale storico di quelle evidenze.

Il risultato non costituisce una certificazione generale di sicurezza: login, OAuth, UAC, sandbox reali, mutazioni account, connettori e filesystem/rete richiedono ancora prove manuali o ambienti esterni.

## Finding 1 — P2: importazione asincrona non piu busy dopo l'ACK — CHIUSO

Stato 2026-09-09: risolto in `p2.rs`. L'identita dell'import resta attiva e
blocca operazioni concorrenti fino alla completion; early completion e
disconnessione post-ACK sono gestite esplicitamente. Le regressioni coprono ACK,
progress, completion, sovrapposizione e incertezza.

Riferimenti principali:

- `src/browser/app_server/p2.rs:314-321`;
- `src/browser/app_server/p2.rs:808-826`;
- `src/browser/app_server/p2.rs:963-981`;
- `src/browser/app_server/p2.rs:987-1057`;
- `src/browser/app_server/p2.rs:1233-1248`;
- `src/browser/app_server/turns.rs:23`.

`Pending::Import` viene rimosso quando arriva la risposta iniziale. Il ramo di successo conserva `import_id` e imposta lo stato `accepted`, ma assegna `migration.busy = false`. Anche il progresso non mantiene il blocco. `p2.busy()` considera le richieste pendenti e i comandi attivi, ma non un'importazione accettata ancora in corso.

Con la sequenza ACK seguito da lavoro asincrono, diventano quindi nuovamente ammesse mutazioni o reconnect prima di `externalAgentConfig/import/completed`. Puo iniziare anche una seconda importazione mentre lo stato conserva un solo `import_id`.

Se la connessione cade dopo l'ACK, `disconnect()` cerca l'incertezza solo in `Pending::Import`, ormai assente, e poi azzera `import_id`. L'importazione precedente puo restare visualizzata come `accepted` o `in_progress` senza un esito riconciliabile.

Impatto:

- perdita del blocco operativo durante una mutazione ancora attiva;
- possibile sovrapposizione di importazioni;
- rappresentazione incompleta dopo disconnect.

Correzione richiesta dall'audit originale (ora applicata):

- trattare `import_id` attivo come busy fino alla completion;
- impedire una seconda importazione o reconnect concorrente;
- trasformare disconnect dopo ACK in stato incerto/da riconciliare;
- testare ACK, progress, completion e disconnect in ordini differenti.

## Finding 2 — P2: terminate rifiutato lascia il comando in `stopping` — CHIUSO

Stato 2026-09-09: risolto. Il rifiuto certo ripristina lo stato controllabile se
il processo e ancora vivo; risposta malformata o perdita di consegna diventano
incerte e disabilitano azioni non sicure; una completion anticipata resta
terminale.

Riferimenti principali:

- `src/browser/app_server/p2.rs:698-714`;
- `src/browser/app_server/p2.rs:1085`;
- `src/browser/app_server/p2.rs:1301-1310`;
- `ui/src/components/NativeP2Settings.svelte:47`;
- `ui/src/components/NativeP2Settings.svelte:252`.

`TerminateCommand` cambia immediatamente lo stato del processo in `stopping`. Se `command/exec/terminate` restituisce un errore RPC, `command_control_finished()` pubblica l'errore e libera `control_busy`, ma non ripristina uno stato controllabile.

Il componente mostra terminate, resize e stdin soltanto quando lo stato e `running`. Lo stato `stopping` impedisce anche un nuovo comando e Clear result. Il controllo Rust rifiuta a sua volta le azioni che richiedono `running`.

Il rendering SSR del componente reale ha confermato lo snapshot:

```json
{
  "terminatedCommandStillShowsStopping": true,
  "terminateControlAvailable": false,
  "stdinControlAvailable": false,
  "clearResultAvailable": false
}
```

Impatto:

- il processo puo continuare senza una via di controllo disponibile nell'interfaccia;
- restano soltanto uscita spontanea, timeout nativo o chiusura dell'applicazione.

Correzione richiesta dall'audit originale (ora applicata):

- distinguere rifiuto certo, accettazione e consegna incerta;
- sul rifiuto certo ripristinare i controlli se il processo e ancora vivo;
- non riaprire uno stato gia completato;
- testare la completion che arriva prima della risposta di terminate.

## Finding 3 — P2: stati di invio residui dopo un errore terminale — CHIUSO

Stato 2026-09-09: risolto nel controller Rust e nella presentazione Svelte.
Feedback, reset ed email non conservano piu copy `uploading`/`requesting` dopo
errori terminali o risposte malformate; gli esiti incerti mantengono solo
l'identita necessaria a un eventuale retry esplicito e sicuro.

Riferimenti principali:

- `src/browser/app_server/p2.rs:838`;
- `src/browser/app_server/p2.rs:876`;
- `src/browser/app_server/p2.rs:891-905`;
- `ui/src/components/NativeP2Settings.svelte:150-154`;
- `ui/src/components/NativeP2Settings.svelte:237-244`.

Il feedback parte con `status = uploading_text_only`. Un errore RPC o una risposta non valida imposta `busy = false` e valorizza `error`, ma non assegna uno stato terminale. Il componente continua quindi a mostrare contemporaneamente "Uploading text-only feedback..." e l'errore.

Lo stesso schema conserva `requesting` per diversi errori di reset ed email. Nel reset, una risposta di successo malformata puo inoltre non conservare la chiave necessaria a rappresentare o recuperare in modo sicuro un tentativo incerto.

Il rendering SSR ha confermato:

```json
{
  "feedbackBusy": false,
  "feedbackShowsUploading": true,
  "feedbackShowsRejection": true
}
```

Impatto:

- interfaccia contraddittoria;
- stato finale non comprensibile;
- recupero incompleto per operazioni che non devono essere ripetute automaticamente.

Correzione richiesta dall'audit originale (ora applicata):

- assegnare uno stato terminale esplicito in ogni ramo di errore;
- rimuovere i testi `uploading`/`requesting` dopo la conclusione del ticket;
- conservare l'identita del tentativo quando l'operazione puo essere stata applicata;
- testare errore, risposta malformata e disconnect in entrambi gli ordini.

## Finding 4 — P2: risposta tardiva sovrascrive settings piu recenti — CHIUSO

Stato 2026-09-09: risolto con una revisione dedicata alle impostazioni. Le
notification piu recenti e le invalidazioni da unload/disconnect non possono
essere sostituite da ACK tardivi di start, resume o fork; entrambi gli ordini
notification/ACK sono coperti.

Riferimenti principali:

- `crates/central-agent-codex-runtime/src/mirror.rs:125-133`;
- `crates/central-agent-codex-runtime/src/mirror.rs:219-247`;
- `crates/central-agent-codex-runtime/src/mirror.rs:264-293`;
- `crates/central-agent-codex-runtime/src/mirror.rs:369-395`;
- `crates/central-agent-codex-runtime/src/conversations.rs:1425`.

`hydrate_session_response()` riceve `requested_at`, ma assegna sempre `target.settings` e `settings_current = true`. Le impostazioni non dispongono di una revisione propria, mentre nome e stato del thread sono protetti da `name_revision` e `status_revision`.

Una notification `thread/settings/updated` puo essere applicata dopo l'inizio di start/resume/fork e prima della risposta. Quando l'ACK tardivo viene idratato, puo sostituire il report piu recente con l'envelope della risposta, perdendo anche campi disponibili solo nella notification. Un report invalidato puo analogamente tornare a sembrare corrente.

Impatto:

- impostazioni visualizzate come correnti ma superate;
- nessuna elevazione automatica del consenso o della sandbox osservata.

Correzione richiesta dall'audit originale (ora applicata):

- aggiungere `settings_revision`;
- rispettare `requested_at` anche per l'inizializzazione delle impostazioni;
- testare notification prima dell'ACK, ACK prima della notification e `notLoaded`/disconnect prima dell'ACK.

## Finding 5 follow-up — P2: receipt paginata riconciliata ma non persistita — CHIUSO

Il primo follow-up Astra ha osservato che `hydrate_paginated_history()` rimuoveva
correttamente la receipt in memoria, mentre l'host emetteva `history_hydrated`
senza risalvare `app-server-threads.json`. Un crash immediato poteva quindi far
ricomparire un avviso gia risolto, senza perdita di conversazione o replay.

La correzione applica l'ordine atomico logico:

1. idratazione e riconciliazione per `clientId` esatto;
2. salvataggio del binding aggiornato;
3. emissione di `history_hydrated` solo dopo il salvataggio.

Se la scrittura fallisce, l'host pubblica l'errore, imposta
`binding_store_error` e blocca nuove azioni: non presenta la receipt come
stabilmente risolta. La recovery acceptance rilegge immediatamente il file e
richiede che la receipt sia assente prima del Resume. Il secondo follow-up Astra
ha riesaminato il ramo e ha emesso **GO tecnico per il finding**, senza nuovi
difetti confermati.

## Verifica specifica del micro-fix Apps 401/403

Riferimenti principali:

- `src/browser/app_server/apps.rs:411`;
- `src/browser/app_server/apps.rs:877`;
- `ui/src/components/NativeApps.svelte:25`;
- `ui/tests/native-apps.test.mjs`.

Il percorso Rust:

- riconosce gli errori HTTP 401/403;
- elimina loading, cursori e strutture transitorie;
- interrompe il seguito della race precedente;
- assegna `current = false`;
- azzera `error` e `notice` generici;
- pubblica soltanto un `unavailable_reason` limitato;
- rende stale eventuali selezioni esistenti;
- ignora risposte il cui identificatore non corrisponde alla richiesta pendente.

Il componente Svelte:

- mostra il motivo di indisponibilita;
- sopprime l'istruzione Resume in presenza di `unavailableReason`;
- mostra "No Apps were reported" soltanto quando `current = true` e la lista e realmente vuota;
- non interpreta l'indisponibilita come catalogo autorevole vuoto.

Sono passati:

- il test Rust `rpc_access_denial_becomes_a_bounded_unavailable_state`;
- il test SSR `Apps access denial is a terminal availability state without stale refresh copy`.

Esito: **micro-fix verificato deterministicamente e visualmente**.

## Controlli deterministici e retest di chiusura

| Controllo | Risultato |
| --- | --- |
| `npm run check` da `ui` | 0 errori e 0 warning Svelte; 68 file modulari e 35 scenari verificati |
| suite `ui/tests/*.test.mjs` | 172 test: 171 passati, 0 falliti, 1 skipped |
| `cargo test --locked --workspace` | 693 passati, 0 falliti, 22 ignored |
| review e permission scope probe | 3 test passati |
| `node scripts/verify-app-server-contract.mjs` | 111 chiamate Rust valide, incluse 17 fixture P0, 11 P1 e 15 P2 |
| `node scripts/verify-provider-reset.mjs` | confine provider verificato |
| `node scripts/verify-inline-js.mjs` | 9 script validi |
| `node scripts/verify-ui-theme.mjs` | controlli statici tema/UI passati |
| `scripts/test-release-pipeline.ps1` | release pipeline probes passati |
| `cargo fmt --all -- --check` | passato |
| `cargo check --locked --workspace --all-targets` | passato |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | passato |
| `cargo audit --ignore RUSTSEC-2023-0071` | nessun advisory bloccante; quattro warning consentiti |
| `scripts/verify-workspace.ps1` dopo i fix | passato integralmente, incluso bundle rigenerato |
| 19 probe runtime opt-in | 15 passate; 4 limiti upstream 0.153.4 osservati e non emulati |
| wire loss fisico main/graph prima/dopo ACK | 4 percorsi passati, nessun replay |
| `cargo build --locked --release --workspace` | passato; entrambi gli eseguibili prodotti |
| smoke hidden-WebView e manifest preview | passati; 5 record verificati, binari `NotSigned` |

I warning non bloccanti rimasti sono registrati come `RUSTSEC-2023-0089`, `RUSTSEC-2024-0370`, `RUSTSEC-2026-0192` e `RUSTSEC-2024-0429`, oltre all'esclusione esplicita `RUSTSEC-2023-0071`. Un audit verde non equivale all'assenza di advisory.

Il rendering SSR mirato e stato ripetuto sui nuovi stati terminali. Non sostituisce
una fault injection desktop, ma verifica il componente reale compilato. Il
wrapper `scripts/verify-workspace.ps1`, non eseguito durante la sola revisione
Astra per conservarne l'indipendenza, e stato poi eseguito integralmente dal
processo principale: ha rigenerato `ui/dist` dal sorgente corrente ed e passato.

## Confini positivi confermati

- il runtime impone 0.153.4;
- il trasporto e stdio;
- `experimentalApi` resta disabilitato;
- `requestAttestation` resta disabilitato;
- l'IPC espone azioni semantiche e non un ponte RPC generico;
- Apps, MCP e comandi conservano l'autorita in Rust;
- i comandi ricevono argv, scope di progetto, policy senza rete e nessun Full access;
- epoche di connessione, owner/thread identity e ticket proteggono i percorsi ordinari esaminati;
- la paginazione della cronologia limita pagine e turni, rifiuta duplicati e pubblica solo al completamento;
- le decisioni native vengono invalidate a risoluzione, completion e disconnessione;
- gli errori Apps 401/403 non installano, selezionano o invocano connettori;
- il warning di model mismatch e informativo e non modifica automaticamente profilo o permessi.

## Controllo visuale annesso

### Ambiente

- eseguibile osservato: `target/debug/central-agent.exe` (build precedente al
  micro-fix non visuale del Finding 5);
- dimensione: 66.508.800 byte;
- SHA-256: `f7d630bc736acd93a9b1f999b5c7f2944fda737a5d642aab82e7405e6bbc7fde`;
- profilo osservato: finestra massimizzata Full HD;
- temi osservati: Dark e Light;
- runtime connesso osservato: Codex App Server 0.153.4;
- account: connessione ChatGPT gia disponibile, senza registrare identificativi personali nel rapporto.

### Percorso visuale eseguito

1. Avvio della build debug allora corrente.
2. Verifica dello stato iniziale disconnected, con CTA `Load Codex history` e telemetria esplicitamente non live.
3. Apertura Settings e connessione dell'App Server.
4. Verifica visibile di `ChatGPT connected` e runtime `0.153.4`.
5. Chiusura Settings e caricamento della cronologia nativa.
6. Verifica della conversazione E2E gia presente, incluse risposte streaming, steering, allegato e rifiuto approvazione registrati nella storia.
7. Apertura della configurazione agente e del lifecycle della conversazione.
8. Resume della conversazione nativa.
9. Verifica del warning di model mismatch tra il modello registrato e quello selezionato.
10. Verifica della nota esplicita: nessun permesso, retry o profilo e stato modificato automaticamente da Central Agent.
11. Apertura di Codex Apps e `Refresh Apps`.
12. Ricezione visuale dello stato 403 di indisponibilita del catalogo.
13. Controllo del testo e della gerarchia in Dark.
14. Passaggio a Light e controllo dello stesso stato.
15. Ripristino del tema Dark originale.

### Esito visuale Apps

Il pannello ha mostrato esclusivamente lo stato:

> Codex Apps are unavailable for this ChatGPT account or workspace. App Server denied access to the catalog; no connector was selected, installed or called.

Non erano presenti:

- `No Apps were reported`;
- `Load Apps after resuming` dopo l'esito 403;
- suggerimenti di Resume;
- notice di race precedente;
- HTML grezzo;
- indicazioni di installazione, selezione o invocazione avvenute.

Il testo era leggibile in entrambi i temi e non presentava sovrapposizioni nell'area osservata.

### Altre osservazioni visuali

- lo stato disconnected iniziale era esplicito e non presentava dati come live;
- dopo la connessione, la UI mostrava la versione runtime corretta;
- il caricamento history non ha avviato un nuovo turno;
- il Resume ha prodotto un warning servizio visibile e separato dalla risposta finale;
- il warning ha indicato chiaramente che Central Agent non aveva cambiato permessi, retry o profilo;
- la configurazione agente distingueva permessi read-only, reasoning summaries, MCP, Apps, hooks e lifecycle;
- il tema Light conservava gerarchia e contrasto sufficienti nel percorso osservato;
- il tema Dark originale e stato ripristinato al termine.
- le superfici P2 neutre per feature, import, account action, feedback e comando
  sono state aperte sul runtime reale in Dark e Light senza clipping,
  sovrapposizioni o scroll orizzontale;
- gli stati terminali P2 di errore/incertezza sono coperti dal rendering
  deterministico del componente reale, non da side effect live eseguiti solo per
  provocare un errore;
- i controlli directory di preferenze, skill e Goal sono coperti end-to-end dai
  relativi host compilati main/graph. Non si dichiara una nuova osservazione live
  di quei pulsanti in una chat regolare priva di conversazione nativa caricata.

Nella cronologia storica e rimasto visibile un frammento simile a protocollo dopo un lungo output numerico. L'analisi precedente lo ha ricondotto al contenuto autorevole della sessione Codex persistita, presente come messaggio assistant/final upstream. Central Agent non aveva concatenato un payload tool. Non e stato introdotto un filtro regex, perche potrebbe corrompere messaggi legittimi. Resta una anomalia dei dati sorgente/upstream da documentare o segnalare, non un difetto confermato di routing della UI.

### Limiti del controllo visuale

Il controllo non ha eseguito:

- login, logout, device code o sessione scaduta;
- OAuth Apps/MCP o catalogo positivo;
- installazione o invocazione di connettori;
- comandi sandbox con effetti, stdin o terminate reali;
- reset crediti, email, feedback o import reale;
- UAC, elevazione o modifica della sandbox Windows;
- picker, audio o cancellazioni permanenti;
- profili 2K e 4K;
- graph agent per ogni stato P2 sintetico.

Questi limiti non invalidano l'esito del micro-fix Apps, ma impediscono di considerare conclusa l'accettazione operativa generale.

## Disposizione finale

**GO tecnico per il sorgente P0/P1/P2 stabile selezionato; NO-GO per una release
pubblica o enterprise.**

Sono completati: chiusura dei cinque finding P2 complessivi, regressioni, bundle, gate
completo, audit visuale autonomo nei limiti dichiarati, build ottimizzata,
smoke hidden-WebView, checksum, inventario e manifest dell'anteprima.

Per il GO pubblico restano soltanto:

1. le validazioni account/OAuth/UAC/sandbox/side effect che richiedono decisione
   e osservazione personali;
2. revisione umana delle licenze e dei quattro warning dependency consentiti;
3. finalizzazione e review del commit, quindi build riproducibile da HEAD pulito;
4. firma Authenticode, registrazione enterprise di `central_agent` e tag finale.

Il micro-fix Apps 401/403 e invece considerato **chiuso e verificato**, salvo la prova futura del percorso positivo con un account/workspace abilitato al catalogo.

## Follow-up indipendente finale GPT-6 Astra

Il revisore Astra ha riesaminato il worktree dopo i fix senza modificare file e
senza eseguire account action, inference o side effect esterni.

Primo passaggio:

- ha confermato chiusi i quattro finding originari su import, terminate,
  feedback/reset/email e settings revision;
- ha confermato i display path Windows di preferenze, skill e Goal;
- ha confermato la correlazione esatta della receipt e la struttura dello
  shutdown concorrente;
- ha individuato il Finding 5 di persistenza host descritto sopra;
- ha corretto i conteggi documentali a 693 test Rust passati/22 ignored nel
  workspace e 171 test frontend passati/1 skipped.

Dopo il fix del Finding 5, il secondo passaggio ha concluso:

> GO tecnico per questo finding: il P2 di persistenza e chiuso. Nessun nuovo
> difetto confermato nel ramo riesaminato.

Astra ha verificato che il ramo e fail-closed in caso di errore di scrittura e
che l'acceptance host rilegge il file prima del Resume. I suoi controlli mirati
sono passati: quattro test `history_pages`, un test exact-client e otto test
`recovery_`. L'host acceptance completa con modello/account non e stata eseguita
nel follow-up, quindi non viene trasformata in evidenza live; il controllo
automatico del sorgente e la regressione su disco restano distinti dalla
validazione personale elencata nell'apposita checklist.

Verdetto consolidato: **GO tecnico per P0/P1/P2 stabile selezionato; NO-GO solo
per distribuzione pubblica/enterprise finche restano i gate esterni e umani
elencati nella Disposizione finale.**
