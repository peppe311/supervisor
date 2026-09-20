# Codex personalization map

Stato verificato il **2026-09-20** con Codex CLI/App Server **0.155.1**. Questa
mappa distingue le personalizzazioni già esposte da Central Agent, quelle solo
osservabili e quelle ancora mancanti. La priorità operativa completa è in
[`docs/CODEX_APP_SERVER_GAP_ANALYSIS.md`](docs/CODEX_APP_SERVER_GAP_ANALYSIS.md).

## Disponibile ora in Central Agent

- **Modello, effort e velocità**: arrivano da `model/list`; la UI mostra solo i
  valori dichiarati dal catalogo dell'account e invia gli identificatori nativi.
- **Reasoning summary per conversazione**: Automatic, Concise, Detailed, Off o
  Inherit. La scelta è locale alla sessione e non espone il ragionamento privato.
- **Codex defaults**: ispezione e modifica confermata di dieci preferenze
  pubbliche (`model`, review model, effort, summary, verbosity, service tier,
  web search, context window e le due impostazioni di compaction). Le scritture
  usano il writer nativo con versione attesa e interessano la configurazione
  Codex condivisa, non soltanto la chat corrente.
- **Impostazioni riportate dal thread**: modello, provider, effort, service tier,
  summary, personality, approval policy e sandbox sono mostrati quando il runtime
  li restituisce. Visualizzare un valore non implica che sia modificabile.
- **Skills native**: inventario per progetto, errori di discovery, attivazione o
  disattivazione confermata e riferimento esplicito alla skill per il prompt
  successivo. Central Agent non legge né esegue autonomamente `SKILL.md`.
- **Compaction e utilizzo**: compaction manuale, stato nativo e telemetria token
  sono separati dalle preferenze e non alterano la capacità del modello.
- **MCP nativo**: inventario, configurazione confermata, reload e OAuth restano
  di proprietà di Codex; nessun server Central Agent viene registrato in modo
  implicito.

## Non ancora integrato come controllo di prima classe

| Capacità | Stato attuale |
| --- | --- |
| Personality capability-aware | Il valore può essere riportato dal thread, ma manca un editor vincolato alle capacità del modello. |
| Upgrade/capability metadata del modello | Non sono ancora proiettati tutti i campi pubblici di provider, upgrade e capability. |
| Custom instructions locali | Nessun editor dedicato; `AGENTS.md` resta una sorgente nativa del progetto, non testo ricostruito dalla UI. |
| Apps/connectors | Inventario, lettura e menzioni native non sono ancora collegati. |
| Hooks | Inventario e ciclo di vita non sono ancora presentati. |
| Operazioni MCP dirette | Lettura risorse, chiamate tool e form OpenAI estesi restano da integrare. |
| Subagent e collaboration modes | Gli eventi nativi noti possono essere presentati, ma i controlli sperimentali non sono abilitati in produzione. |

## Confini da preservare

- Autenticazione, cronologia, tool, permessi e approvazioni native appartengono
  ad App Server. Central Agent conserva binding e ricevute, non ricostruisce una
  seconda cronologia Codex.
- Il grafo di progetti e agenti resta una superficie locale separata. Non
  aggiunge automaticamente contesto o strumenti alle conversazioni Codex.
- Modello, stile e velocità non ampliano mai accesso a file, shell, rete, MCP o
  servizi esterni.
- Le API sperimentali restano disabilitate (`experimentalApi: false`) finché non
  esiste una decisione di prodotto, sicurezza e compatibilità dedicata.
- Non hardcodare modelli, effort, tier o personality: disponibilità e capacità
  dipendono da runtime, account e policy gestite.

## Prossimi incrementi consigliati

1. Chiudere prima i gap P0 di correttezza e sicurezza: notice azionabili,
   riconciliazione/unsubscribe dei thread, inventario dei permission profile e
   aggiornamento live dei rate limit.
2. Aggiungere personality e metadata modello solo quando la capability è
   dichiarata dal runtime e l'effetto è visibile prima dell'invio.
3. Integrare Apps, hooks e operazioni MCP dirette come superfici native, senza
   introdurre un secondo tool loop.
4. Mantenere subagent e collaboration modes dietro un gate esplicito finché la
   relativa API resta sperimentale.

Fonti ufficiali: [App Server](https://learn.chatgpt.com/docs/app-server),
[modelli Codex](https://learn.chatgpt.com/docs/models),
[configurazione](https://learn.chatgpt.com/docs/config-file/config-reference) e
[personalizzazione](https://learn.chatgpt.com/docs/customization/overview).
