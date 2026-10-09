# Research: terms of use for reading the own usage, and the ways to split the token use

Status: working note of 2026-10-08. Two parts: what the terms of use say about the account source (REQ-124), and by which dimensions the token use can be split (REQ-121).

## Part 1: terms of use and the account source (REQ-124)

### What was checked

The two primary pages were fetched on 2026-10-08: the Claude Code page "Legal and compliance" (https://code.claude.com/docs/en/legal-and-compliance) and the Consumer Terms of Service (https://www.anthropic.com/legal/consumer-terms, effective date shown: October 8, 2025). The quotes are word for word. For a Pro subscription the Consumer Terms apply (the page says: "Consumer Terms of Service - for Free, Pro, and Max users").

### The clauses

| Clause | Wording | Source |
|---|---|---|
| Automated access (a prohibited use) | "Except when you are accessing our Services via an Anthropic API Key or where we otherwise explicitly permit it, to access the Services through automated or non-human means, whether through a bot, script, or otherwise." | Consumer Terms, section on prohibited uses |
| Scraping | "To crawl, scrape, or otherwise harvest data or information from our Services other than as permitted under these Terms." | Consumer Terms |
| Account credentials | "You may not share your Account login information, Anthropic API key, or Account credentials with anyone else or make your Account available to anyone else." | Consumer Terms |
| Reverse engineering | "To decompile, reverse engineer, disassemble, or otherwise reduce our Services to human-readable form, except when these restrictions are prohibited by applicable law." | Consumer Terms |
| Purpose of the sign-in token | "OAuth authentication is intended exclusively for purchasers of Claude Free, Pro, Max, Team, and Enterprise subscription plans and is designed to support ordinary use of Claude Code and other native Anthropic applications." | Claude Code, Legal and compliance |
| Third-party developers | "Anthropic does not permit third-party developers to offer Claude.ai login into their own applications, or to route requests through Free, Pro, or Max plan credentials on behalf of their users. Moreover, developers may not collect, store, or intermediate Claude.ai credentials or session tokens — sign-in to a Claude account must complete through Anthropic's own flow." | Claude Code, Legal and compliance |
| Enforcement | "Anthropic reserves the right to take measures to enforce these restrictions and may do so without prior notice." | Claude Code, Legal and compliance |
| Usage limits | "Advertised usage limits for Pro and Max plans assume ordinary, individual usage of Claude Code and the Agent SDK." | Claude Code, Legal and compliance |

### Reading (not legal advice; confidence medium)

- **The clause on automated access is the one that matters, and it points against the plan.** A program of our own that calls the usage endpoint with the subscription's sign-in token is access to the Services "through a script". The clause allows that only for an Anthropic API key or where Anthropic "explicitly" permits it. For the usage endpoint no explicit permission was found (the endpoint is not documented). By the letter, the plan of REQ-124 therefore falls under the prohibition. A reading that this clause does not prohibit the use does not fit its wording.
- The clauses on third-party developers aim at products that offer sign-in or collect credentials of other people. A program that reads only its owner's own token on the owner's own computer is not that case in the letter, but it also reads a token that is "designed to support ordinary use of Claude Code and other native Anthropic applications".
- **The status line is not affected.** There Claude Code itself runs the command and hands over the numbers; the program makes no request to the Services.
- **Transcript files are not affected:** reading files on one's own disk does not access the Services.
- **Consequence stated by the terms:** measures "without prior notice". What measure, and whether it has been applied to tools that only read the own usage, is not known (community reports name none; that is not a guarantee).
- **The calls of 2026-10-08** with the owner's stored token (HTTP 200, both windows) belong to the same class.

### What this means for the requirement

REQ-124 (account source with the stored token) rests on an undocumented endpoint and, by the letter of the Consumer Terms, on access that is not explicitly permitted. The owner decides D3 knowing that. Options:

1. Do not build REQ-124. Live percentages remain what the status line gives (REQ-011, terminal client only) and the live token counts from the transcript files (REQ-122) carry the "live" view.
2. Build REQ-124 as an option that is **off by default**, switched on only by the owner with a clear text about the risk, so that nobody uses it unknowingly.
3. Ask Anthropic first (the page names this for questions on permitted authentication methods: "contact sales") and build only after an explicit permission or a documented interface.

Recommendation: 3, in parallel with 1; 2 only if the owner takes the risk knowingly.

## Part 2: ways to split the token use (REQ-121)

### The source: transcript files of Claude Code

Measured on 2026-10-08 on the owner's computer (field names and counts only; no message text, ids or paths were read out): 247 files (104 of them files of sub-agents), about 167,000 lines, 55,569 assistant messages with token counts, 142 sessions, 23 project folders, 145 different working directories, 259 git branches, 37 Claude Code versions. The format is not documented and has changed over time.

### What every assistant message carries

- **Token counts:** `input_tokens`, `output_tokens`, `cache_creation_input_tokens` (with the split `ephemeral_5m_input_tokens` and `ephemeral_1h_input_tokens`), `cache_read_input_tokens`, `output_tokens_details.thinking_tokens` (thinking part of the output), `server_tool_use` (`web_search_requests`, `web_fetch_requests`).
- **Context:** `model` (here 8 values, among them one `<synthetic>` with no tokens), `service_tier`, `speed`, `inference_geo`, `effort` and `perTurnEffort` (the effort level), `advisorModel`.
- **Where it was made:** `sessionId`, `cwd` (the working directory, so the project), `gitBranch`, `entrypoint` (`claude-vscode`, `claude-desktop`, `cli`, `sdk-cli`), `version`, `isSidechain` (true for work of sub-agents), `agentId`.
- **What it belongs to:** `promptId` (the prompt of the person that started the turn), `requestId` and the message id (the key against double counting), `parentUuid` (the chain), and `attributionSkill`, `attributionAgent`, `attributionMcpServer`, `attributionMcpTool` (which skill, agent or tool server caused it).
- **What it did:** the names of the tools used in the message (in this data `Bash`, `Read`, `Edit`, `PowerShell`, `Write`, `Grep`, `WebSearch`, `WebFetch` and others).
- **When:** the time stamp, so hour, weekday, week, month.

### Answer to the questions

| Question | Answer |
|---|---|
| Per model, as in the present tool? | **Yes.** The present tool shows tokens per model (four kinds) from these files; the new database keeps it. The **percentages** of the account (5 h and 7 d) are not per model: the status line has only the two windows. The account endpoint has fields for model specific weekly figures (`seven_day_opus`, `seven_day_sonnet`), which were empty (`null`) on this account at the time of the call. |
| Per session? | **Yes**, by `sessionId`; a readable name is only available as a title text (see privacy below). |
| Per project? | **Yes**, by the working directory (`cwd`) or by the project folder of the transcript file; branch by `gitBranch`. |
| Further splits | Per client (VS Code extension, desktop, terminal, SDK), per Claude Code version, per sub-agent (`isSidechain`), per effort level, per tool used, per skill, per agent or tool server, per prompt (`promptId`), thinking share of the output, 5 minute and 1 hour cache writes, web searches and fetches, service tier and speed, hour of day and weekday. |
| Cost in money | Not in the messages. A price table is needed to compute it; the status line hands over an estimate of the session cost (`cost_usd`), and the files hold a few `cost-state` entries. Prices change, so a computed cost would be an estimate and labelled so. |
| Several computers | Each computer has its own files. The account percentages are for the account. The import of REQ-123 adds the other computers' counts, labelled with the computer name. |

### Limits and cautions

- **Not an invoice.** The counts are what Claude Code wrote; the double entries of streamed messages are removed by the message id and request id (as the present tool does).
- **Attribution to a tool or a skill is approximate:** a message that uses two tools cannot be split exactly; the tokens would be counted for the message as a whole and listed by tool as "messages that used the tool".
- **Privacy:** the files also hold titles of sessions (`aiTitle`, `customTitle`, `slug`, `lastPrompt`) and the text of the conversation. The database should keep **counts, ids, times, model and project names only** and no message text; whether the session title is kept (it is text written about the content) is a decision of the owner. The files hold account identifiers (`ownerAccountUuid`, `ownerOrganizationUuid`) in some lines; they are not needed and are not stored.
- **Format risk:** the fields are not documented by Anthropic. A new version of Claude Code can rename or drop one; the reading stays tolerant (an unknown field is ignored, a missing count is 0), as the present code does.

### Suggested set of views for REQ-121

Time (hour, day, week, month, free range), model, project, session, branch, client, sub-agent or main, effort level, tool, skill, token kind (input, output, cache write 5 min, cache write 1 h, cache read, thinking), and the sums and shares of them. Further views can be added later from the same table without changing the stored data, because every message is stored with all these fields.
