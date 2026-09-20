# Piano di integrazione Agents API in Supervisor

Data: 11 settembre 2026. Stato: **proposta tecnica**, successiva allo [studio](OPENAI_AGENTS_API_STUDY.md). Fonti: [documentazione ufficiale](https://developers.openai.com/api/docs/guides/agents-api/overview); le copie integrali restano riferimenti locali esclusi dalla pubblicazione.

## Obiettivo del primo incremento

Aggiungere una sessione cloud opzionale capace di ricevere un prompt, mostrare correttamente il lavoro del coordinatore e di due sottoagenti, recuperare la storia dopo il riavvio e interrompere il turno. Primo caso: revisione di due testi forniti esplicitamente come input, senza accesso automatico al repository.

Il provider Codex attuale conserva conversazioni, profilo, fork, impostazioni e runtime 0.153.4. Una sessione Agents API ha un'identità e credenziali proprie. Il piano non prevede una migrazione automatica delle conversazioni esistenti.

## Confine architetturale

Introdurre un provider persistente distinto, proposto come `openai_agents_api`, con un client Rust HTTP/SSE. Il ciclo modello/strumenti resta al harness OpenAI. La UI riceve azioni semantiche e viste del lavoro dal backend.

| Area | Punto attuale da considerare | Intervento proposto |
| --- | --- | --- |
| Selezione provider | `src/provider.rs`, `src/provider_types.rs` | Nuova identità e capacità; non riutilizzare l'adattatore subscription |
| Trasporto | `crates/central-agent-codex-runtime/src/transport.rs` | Nuovo modulo/crate HTTP/SSE separato dal JSON-RPC stdio |
| Lifecycle conversazione | `src/browser/app_server/turns.rs`, `lifecycle.rs` | Nuovo host Agents API, con invii persistenti e gestione dello stato remoto |
| Persistenza | `src/browser/app_server/native_profile.rs`, `chat_reload.rs` | Archivio delle nuove sessioni separato; ricaricamento automatico per provider |
| Timeline | `mirror.rs`, `ui/src/lib/native-history.ts`, `NativeConversation.svelte` | Proiezione specifica Agents API e interfaccia comune minima, senza fingere un `Thread` nativo |
| Albero | `src/browser/chat_ownership.rs`, `ui/src/lib/chat-tree.ts`, `ProjectChatTree.svelte` | Relazioni esplicite tra sessione, coordinatore e sottoagenti |
| Azioni pendenti | `src/browser/app_server/requests.rs` | Nuovo registro delle required actions e handler delle funzioni; riuso solo della presentazione compatibile |
| Strumenti locali | Non esposti | Valutare un accesso limitato mediante executor in un incremento successivo, senza ripristinare il server MCP locale rimosso |
| Diagnostica | `src/browser/app_server/event_log.rs` | Stesso criterio di metadata limitati, con nomi e identità del protocollo API |

Il nome del nuovo modulo/crate e la forma dell'interfaccia condivisa sono scelte da consolidare durante l'implementazione. Non introdurre un terzo framework frontend. Qualunque futura modifica visiva deve seguire `DESIGN.md`; i controlli via Computer Use rimangono esclusi senza richiesta dell'utente.

## Identità e persistenza

La chiave di un binding deve includere provider e progetto API, oltre all'owner locale. Conservare almeno:

| Dato | Uso |
| --- | --- |
| `owner` e progetto Supervisor | Destinazione locale, chat o scheda del grafo |
| Identità del profilo API e progetto Platform | Evitare letture e operazioni con credenziali di un altro progetto |
| `session_id` | Risorsa cloud durevole |
| `agent_id` della sessione e `subagent_id` eventuale | Attribuzione del lavoro; distinto dall'ID di un profilo agente riutilizzabile |
| `environment.id`, tipo e `remote_url` eventuale | Connessione e gestione dell'ambiente |
| Configurazione risolta e revisioni locali | Mostrare ciò che è realmente attivo |
| Intento dell'invio, chiave idempotente e stato della consegna | Recuperare invii incerti senza generarne di nuovi |
| Stato di archiviazione locale ed eliminazione | Separare organizzazione della UI e cancellazione remota |

La chiave API appartiene allo storage sicuro delle credenziali, non al binding o alla WebView. Creare e salvare l'owner prima di pubblicare una sessione nell'albero. Non ricostruire relazioni da titoli, ordine di arrivo o cartelle.

Fonti: [sessioni](https://developers.openai.com/api/docs/guides/agents-api/sessions), [subagents](https://developers.openai.com/api/reference/typescript/resources/beta/subresources/agents/subresources/sessions/subresources/subagents/methods/retrieve), [sandbox self-hosted](https://developers.openai.com/api/docs/guides/agents-api/environments/self-hosted).

## Contratto minimo da implementare

Tutti i percorsi della tabella sono relativi a `/v1`. Le richieste richiedono l'autenticazione Platform e `OpenAI-Beta: agents=v1`.

| Operazione | Contratto pubblico |
| --- | --- |
| Crea sessione e primo input | `POST /agents/sessions`, con `stream: true` quando opportuno |
| Leggi stato autorevole | `GET /agents/sessions/{session_id}` |
| Segui eventi live | `GET /agents/sessions/{session_id}/events` |
| Invia o indirizza il lavoro | `POST /agents/sessions/{session_id}/events`, evento `agent.session.input.message` |
| Richiedi stop | Stesso endpoint, evento `agent.session.input.cancel` |
| Rispondi a una funzione | Stesso endpoint, evento `agent.session.input.tool_result` con `turn_id`, `call_id` e risultato |
| Carica storia principale | `GET /agents/sessions/{session_id}/items`, paginato, con filtro `turn_id` se necessario |
| Leggi esiti | `GET /agents/sessions/{session_id}/turns` e `.../turns/{turn_id}` |
| Carica sottoagenti e relativa storia | `.../subagents`, `.../subagents/{subagent_id}/items`, turni e items per turno |
| Elimina sessione | `DELETE /agents/sessions/{session_id}`; verificare la conferma restituita |

La reference rende l'invio eventi come `void`, non come una risposta del modello. Il parametro TypeScript `idempotencyKey` è documentato per il retry dei messaggi: verificare la mappatura header della versione SDK/protocollo scelta e i limiti della garanzia prima di implementare retry automatici in Rust. La creazione iniziale non espone la stessa opzione nella reference esaminata. [Invio eventi](https://developers.openai.com/api/reference/typescript/resources/beta/subresources/agents/subresources/sessions/subresources/events/methods/create), [creazione sessione](https://developers.openai.com/api/reference/typescript/resources/beta/subresources/agents/subresources/sessions/methods/create).

## Proiezione degli eventi

- Separare stato della sessione, del turno, del sottoagente e dell'ambiente. Sono quattro lifecycle diversi.
- Ogni aggiornamento va al binding corretto anche quando l'utente cambia chat.
- Conservare `event_id` per deduplica osservativa, senza usarlo come cursore di replay o ordinamento temporale.
- Testo indicizzato per sessione/turno/item/parte; riassunti indicizzati per item/summary index. Un `done` autorevole sostituisce i delta temporanei.
- I tipi pubblici di coordinamento sono `create_subagent_call`, `send_subagent_input_call`, `wait_for_subagents_call`, `interrupt_subagent_call` e, quando disponibile, `agent_message`.
- Il dashboard può mostrare nomi interni diversi: non usarli come discriminanti del parser pubblico.
- Un item sconosciuto resta rappresentabile con metadata limitati; non deve trasformarsi in testo finale o autorizzare strumenti.

Per il recupero, seguire il flusso stream aperto → buffer → stato e storia paginata → riconciliazione → live. Non riaprire lo stream supponendo che consegni automaticamente ciò che manca. [Eventi](https://developers.openai.com/api/docs/guides/agents-api/sessions/events), [reference degli eventi](https://developers.openai.com/api/reference/resources/beta/subresources/agents/streaming-events).

## Capacità da esporre per provider

| Controllo | Agents API nella beta esaminata |
| --- | --- |
| Nuova sessione, continuità e cronologia | Disponibili |
| Indirizzare il turno attivo | Disponibile dove il turno è steerable; gestire il rifiuto esplicito |
| Stop del turno principale | Disponibile tramite input cancel |
| Osservazione dei sottoagenti | Disponibile; mutazioni dirette del figlio non esposte dagli endpoint esaminati |
| Fork nativo, rollback, goal del runtime | Nessun equivalente documentato nella superficie esaminata |
| Cambio live di modello/tools/effort | Session update espone solo metadata; proposta: nuove impostazioni per una nuova sessione |
| Archiviazione | Stato locale di Supervisor, distinto dalla cancellazione API |
| Trace completa | Dashboard esterno; nessun exporter pubblico di trace nella beta |

Disabilitare soltanto i controlli non disponibili per la sessione selezionata e spiegare il motivo con linguaggio di prodotto. Nessun inoltro degli ID cloud alle funzioni `thread/*` dell'App Server. [Update session](https://developers.openai.com/api/reference/typescript/resources/beta/subresources/agents/subresources/sessions/methods/update), [osservabilità](https://developers.openai.com/api/docs/guides/agents-api/observability).

## Ordine degli incrementi

### 0. Studio e fonti — completato

Data del rilascio verificata; 82 documenti salvati; confronto con il codice e disponibilità locale di `exec-server --help` controllati. Questa fase non richiede una chiave API.

### 1. Sessione cloud minima

Implementare configurazione separata delle credenziali, binding, HTTP/SSE, cronologia, invio, stop, recupero e attribuzione dei sottoagenti. Avviare il primo caso con `environment.type: "none"` e materiali testuali forniti nel prompt. Conservare la selezione economica dell'utente se supportata dal progetto: Luna, effort low, tier default. Rendere la delega esplicita e limitarla a 2 nel test iniziale.

Criterio di uscita: il risultato del test è corretto; riavvio e disconnessione non duplicano prompt o messaggi; i sottoagenti compaiono sotto il coordinatore; gli errori non vengono presentati come completamento riuscito.

### 2. Funzioni del coordinatore e MCP

Un handler Rust legge le `required_actions` correnti. Per le funzioni conserva esito e identità della chiamata prima di inviare il risultato. Le autorizzazioni restano legate ad azione e task; una richiesta ricostruita dalla sola storia non viene eseguita.

Per strumenti richiesti dai sottoagenti scegliere MCP, dato che le functions non sono supportate dai figli. Collegare inizialmente solo strumenti di lettura selezionati. Il catalogo completo della memoria locale non viene esposto automaticamente a una sessione remota. [Funzioni](https://developers.openai.com/api/docs/guides/agents-api/tools/functions), [MCP](https://developers.openai.com/api/docs/guides/agents-api/tools/mcp).

### 3. Ambiente e file

Usare prima un sandbox OpenAI per un report riproducibile. Pubblicare sotto `/workspace/outputs` e scaricare gli artifacts corrispondenti a turno e percorso.

Valutare poi l'executor sul compute proprio, con un ciclo di vita separato dal runtime nativo: processo in background, environment key dedicata, filesystem circoscritto e ambiente ID/URL restituiti dal servizio. La versione già usata dall'App Server non viene aggiornata implicitamente. Eseguire una verifica specifica Windows e gestire `executor_version_incompatible`. [Artifacts](https://developers.openai.com/api/docs/guides/agents-api/environments/files), [self-hosted](https://developers.openai.com/api/docs/guides/agents-api/environments/self-hosted).

### 4. Lavori persistenti e osservabilità

Per continuare con il desktop chiuso, ospitare altrove l'eventuale handler e il compute necessari. Aggiungere webhook con verifica della firma, deduplica e lettura dello stato corrente. Distinguere `agent.session.action_required` dei webhook da `agent.session.requires_action` dello stream.

Mostrare usage come dato parziale, includendo i figli senza sommare due volte totali già aggregati. Un limite di concorrenza è una misura operativa, non un tetto economico. Nessuna promessa di risparmio senza confronto su task equivalenti. [Webhook](https://developers.openai.com/api/docs/guides/agents-api/sessions/webhooks), [usage](https://developers.openai.com/api/docs/guides/agents-api/observability).

## Prove di accettazione necessarie

Le prime prove possono usare fixture del contratto senza modello. Le prove reali richiedono credenziali Platform con le capacità corrette; il login Codex già collegato non le sostituisce.

| Prova | Evidenza richiesta |
| --- | --- |
| Cambio chat durante lo stream | Aggiornamenti sempre associati all'owner originario |
| Eventi di coordinatore e figli alternati | Nessuna fusione del testo e nessuna chiusura prematura del turno principale |
| Delta mancanti e `done` completo | Risposta completa una sola volta, senza concatenazione duplicata |
| Riassunti multipli | Indici distinti, nessun contenuto cifrato mostrato come reasoning |
| Disconnessione con eventi persi | Recupero da tutte le pagine e riconciliazione degli items terminali |
| Invio dall'esito incerto | Chiave e payload stabili; nessuna nuova sessione o doppio invio cieco |
| Function già eseguita prima del crash | Risultato salvato riutilizzato; effetto eseguito una sola volta |
| Required action storica o risolta | Nessuna nuova esecuzione |
| Sottoagente `active` ma senza turno in corso | Stato disponibile distinto da lavoro in esecuzione |
| `idle`, turno fallito e tool fallito | Esito leggibile e corretto, senza falso successo |
| Uso sconosciuto o tardivo | `null` mantenuto come sconosciuto; aggiornamento senza doppio conteggio |
| Stop e cancellazione | Stop confermato dal turno; delete confermato dalla risorsa; compute self-hosted gestito separatamente |
| Credenziali di progetto diverso | Operazione rifiutata, senza cambiare il binding locale |
| Executor scollegato | Nessun reinvio mentre la richiesta iniziale attende; timeout e riconnessione trattati distintamente |

Non sono stati eseguiti test visivi tramite Computer Use. Il piano riguarda test di contratto e, nella fase d'integrazione, chiamate API mirate con fixture temporanee.

## Questioni aperte da chiudere nel prototipo

1. Accesso effettivo alle Agents API e al modello nel progetto Platform scelto.
2. Compatibilità del percorso executor Windows con il servizio corrente; `--help` non la dimostra.
3. Semantica completa dell'idempotenza dei messaggi e gestione della creazione dall'esito incerto.
4. Scelta del client HTTP/SSE Rust e contratto pubblico fissato alla versione verificata.
5. Perimetro degli strumenti e dei materiali da rendere disponibili ai task cloud.

L'implementazione dovrebbe iniziare dall'incremento 1. È il passaggio più piccolo che verifica il valore delle nuove API per Supervisor prima di introdurre accesso ai file locali o gestione di infrastruttura.
