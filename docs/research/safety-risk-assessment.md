# Safety Risk Assessment: Harm Scenarios for Agent Financial Autonomy

**Wayfinder Research Ticket #5 — Part of #1 (AI Bank: Decentralised credit network for LLM agents)**
**Branch:** `research/safety-risk-assessment` | **Date:** 2026-09-02 | **Author:** Muse Spark (research subagent)
**Status:** Research complete — decision-ready
**Dependency note:** This is the foundational safety decision. It blocks #6 (safety documentation pattern) and #7 (safety evaluation framework), and informs ledger/reputation design. Maintainer governs safety decisions (lightweight process per #1). Safety is part of MVP (issue #1).

---

## TL;DR for Decision-Maker

| Question | Answer |
|---|---|
| What can agents do with money that causes harm? | 12 concrete harm families (see §2). Five are high-severity even at MVP: Sybil farming, collusion rings, reputation gaming, resource-exhaustion flooding, and funding harmful tasks. All are enabled by trivial agent scale (one operator → 1000 agents in seconds). |
| Analogous systems | P2P payment networks (Venmo/CashApp fraud), Bitcoin/Monero double-spend & eclipse, Ethereum/Solana MEV & wash trading, Napster/BitTorrent pollution, AutoGPT/Eliza agent marketplaces (prompt-injected fund drainage), Lightning Network griefing. Every harm has a deployed precedent. |
| RSP template | Anthropic RSP ASL-1..4 maps cleanly to **Financial Autonomy Levels (FAL)**. FAL-1 (no-risk sandbox) → FAL-4 (autonomous macro-economy). Each level = capability threshold + required safeguards (§5). |
| MVP blast radius (fixed-supply virtual credits, accounts+transfers+reputation, no escrow/lending/fiat bridge) | **Contained.** No real-world financial loss, no legal tender, no irreversible harm. Worst case = ledger corruption + reputation collapse + denial-of-service within the network. Recoverable by genesis reset. Does *not* reach "catastrophic" in RSP sense. |
| Recommendation | Adopt **4-level FAL** (FAL-1..4). Ship MVP at **FAL-2** with proportionate safeguards listed in §5. Require explicit maintainer review + evaluation gate before any FAL-3 feature (escrow, lending, fiat bridge, cross-network credit). |

**Bottom line:** Keep virtual, fixed-supply, no-leverage. That single design choice caps blast radius. Reputation is the main attack surface at MVP. Define FALs now so future features cannot slip in without a safety gate.

---

## 1. How Anthropic RSP ASL Works — Template to Copy

Primary source: Anthropic RSP v3.4 (2026-07-08) and underlying v1.0–v3.1 PDFs at `anthropic.com/responsible-scaling-policy` and `cdn.sanity.io` / `www-cdn.anthropic.com`. Key evolution tracked across search excerpts.

### 1.1 Core structure

- **AI Safety Levels (ASL Standards)** are *sets of safeguards*, not model labels (terminology tightened in v2.0, Oct 15 2024). Each ASL Standard = technical + operational measures for a capability band. As capability ↑, required safeguards ↑.
- Two current categories: **Deployment Standards** (prevent misuse after release) and **Security Standards** (prevent weight/IP theft). Optional third in later drafts: internal governance.
- **Capability Thresholds** trigger upgrade: when a model crosses a threshold, the next ASL Standard becomes *required* before training/deployment may continue. Thresholds are *re-evaluated whenever safeguards are upgraded* (v2.1, Mar 31 2025 added commitment to re-evaluate thresholds on every upgrade).
- Modeled loosely on **US BSL (Biosafety Levels)** for dangerous pathogens. Explicit: temporary *pause* if scaling outstrips ability to comply, creating a "race to the top" incentive to solve safety to unlock scaling.

### 1.2 ASL definitions (RSP v1.0 summary + later refinements)

| ASL | RSP definition (paraphrased from source, Sep 19 2023 post + pdfs) | What triggers it |
|---|---|---|
| **ASL-1** | Systems posing **no meaningful catastrophic risk** (2018-era LLM, chess-only AI). Baseline infosec only. | Pre-threshold |
| **ASL-2** | Systems showing **early signs of dangerous capabilities** (e.g., can give bio-weapon instructions) but not yet *usefully* beyond search engines / unreliable. Current LLMs including Claude at RSP launch were ASL-2. | Early dangerous capability, low reliability |
| **ASL-3** | Systems that **substantially increase catastrophic misuse risk vs non-AI baselines** (search/textbooks) **OR show low-level autonomous capabilities**. Requires unusually strong security, world-class red-team, and *show-no-meaningful-catastrophic-risk* under adversarial testing; if not met, no deployment. | 6× scaling buffer heuristic, then capability evaluations every 6 months (v2.0 extended from 3→6 mo) |
| **ASL-4+** | Undefined at launch ("too far"), expected **qualitative escalation** in misuse + autonomy. Requires *unsolved research* e.g. mechanistic interpretability proof that model won't cause catastrophic harm. | Defined before reaching ASL-3 (commitment in v1.0; relaxed in v2.1 to "reconsider thresholds on upgrade") |
| **v2.1–v3 additions** | New CBRN-2 threshold (uplift for moderately resourced state programs), split AI R&D thresholds (entry-level researcher automation vs dramatic scaling acceleration — "compress 2 yr of 2018–2024 progress into 1 yr"), **Frontier Safety Roadmap** + **Risk Reports** every ~6 months + external review (v3, Feb 24 2026). | AI R&D automation |

**Governance (RSP v1→v3.4):** Board approval for RSP changes (Long-Term Benefit Trust consultation), Responsible Scaling Officer, internal critique + external expert input, public `rsp-updates` page, redacted Risk Reports, external reviewer access, noncompliance reporting + anti-retaliation (Feb 2026 policy), LTBT can request external review (v3.2, Apr 29 2026).

Source fidelity notes:
- ASL-1..3 summary text is from the primary announcement post `anthropic.com/news/anthropics-responsible-scaling-policy` (Sep 19 2023) verbatim. Full thresholds in Appendix C of the Oct 15 2024 / Mar 31 2025 PDFs (gated — excerpted via search).
- The v2.0 *ASL definition change* (safeguards, not models) and *pause incentive* are from the same PDFs, changelog sections.
- Deployment safeguards for ASL-3 (four-layer defense-in-depth: access controls, real-time classifiers, async monitoring, post-hoc jailbreak rapid response) and 17 Security safeguards (access compartmentalization, researcher tooling, SLSA, binary auth, patching, Executive Risk Council, multi-party weight access, IaC, CSPM, red-teaming, SIEM/SOAR, deception/honeypot weights) are from the Oct 15 2024 planned-safeguards page excerpt above.

### 1.3 What AI Bank should copy

- Levels = **safeguard bundles tied to reversible triggers**, not vibes.
- **Pause rule:** if you cannot meet the next level's safeguards, you *pause* that feature's deployment.
- Public commitments + periodic risk reports + external review + non-retaliation for raising concerns.
- Thresholds are *revisited* when safeguards change — avoids rigidity as threat model evolves.

---

## 2. Concrete Harm Scenarios: What Agents Can Do with Financial Autonomy

Each row: scenario → mechanism → MVP-relevant? → analogous precedent → mitigations that exist.

| # | Harm family | Concrete scenario (agent-centric) | MVP (FAL-2) relevant? | Analogous precedent (primary) |
|---|---|---|---|---|
| 1 | **Sybil / identity forgery** | One operator spins 500 agents, each with fresh Ed25519 PeerId, farms genesis credits via airdrop or referral bonus, or gets N× voting weight. 2026 surge in AI-forged PoP noted in literature. | **YES — central.** MVP issues identity as keypair; no proof-of-personhood. Cost of new identity ≈ 0. | Bitcoin P2P eclipse via Sybil nodes (Douceur 2002, Microsoft Research; Ledger/GeeksforGeeks histories); Twitter 44M fake accounts/month (2022); 2025–26 analyses: rule-based post-hoc Sybil detection yields high false positives and misses industrial farming [MDPI Applied Sciences 16(14):6929, 2026]. |
| 2 | **Collusion rings / cartels** | 5 agents owned by same principal circularly trade to inflate each other's reputation scores, then jointly defraud a 6th honest agent on a high-value task. | **YES** | Ethereum wash trading / MEV collusion; BitTorrent Sybil pollution; eBay/Amazon review rings. |
| 3 | **Reputation gaming (whitewashing + slandering)** | Agent builds high reputation via many tiny honest transfers (cost ≈ 0 with virtual credits), then defects once on a large transfer; or agents mutually boost with zero-value transfers; or an agent whitewashes by discarding a tainted PeerId and regenerating a clean keypair (identity cheap → history cheap). Negative-reputation slandering: coordinated bad reviews after honest transfer fails due to counterparty fault. | **YES — primary attack surface at MVP.** | Halborn 2026 "How Attackers Game AI Agent Reputation Systems (and How to Stop Them)" — lists exactly these patterns; P2P reputation grant by tenure/age is *known* defense but exploitable via age-farming. |
| 4 | **Funding harmful tasks / resource brokering** | Agent with credits pays another agent to perform disallowed work: scrape past rate limits, run CAPTCHAs, generate spam/phishing, buy compute for password cracking, pay for data exfiltration. Credits become a generic capability multiplier. | **YES but bounded** (virtual-only). No fiat, no real-world purchase, but *within* the agent economy it is fully fungible. Shows why future fiat bridge escalates risk sharply. | AutoGPT marketplaces & Eliza agent frameworks: prompt-injected fund drainage to third-party tools; P2P payment fraud (Venmo/Cash App) funding illicit services. |
| 5 | **Resource exhaustion / DoS** | Agent floods network with dust transfers (1-credit × 10K), swelling ledger, forcing all nodes to replicate; or spams gossipsub with invalid signatures; or opens many relay reservations to exhaust relay caps. Replicated ledger + gossip amplify one spammer to all nodes. | **YES** | Bitcoin dust/UTXO bloat; Solana spam-induced halts; libp2p relay `Limit{duration,data}` is explicit DOS control (circuit-v2 spec) — *because* this happens. |
| 6 | **Ledger forks / double-spend / inconsistency** | Without consensus, two nodes see conflicting histories (e.g., agent double-spends limited credits by sending to A and B on partitioned halves). Reconciliation rule determines winner; loser loses funds. Fixed-supply invariant can break. | **YES** | Bitcoin double-spend taxonomy: race (zero-conf), Finney (private block), chain-reorg, 51% majority [Changelly 2026; Investopedia 2026; bitcoin.org whitepaper]. AI Bank MVP has *no chain* — risk is forked replication, same shape. |
| 7 | **Credit inflation / unauthorized minting** | Bug or compromised node mints beyond fixed supply; or genesis allocator exploits initial distribution. Because credits are virtual, inflation is not externally audited (no L1 anchor). | **YES (genesis-specific)** | Ethereum token mint bugs (e.g., 2018 BeautyChain inflation), Solana token-2022 bugs; analogous to ledger integrity failure. |
| 8 | **Economic manipulation / market cornering** | Agent hoards credits (fixed supply → scarcity), creates artificial scarcity, demands premium for tasks; or coordinated dump crashes perceived credit value (if credits priced tasks). No monetary policy lever at MVP. | Partial (network small) | Crypto market cornering, GameStop-style squeezes; Bitcoin fixed supply (21M) → hoarding dynamics. |
| 9 | **Griefing / escrow-free fraud** | Agent requests paid work, receives result, refuses to pay (or pays then claims non-delivery). Without escrow, there is no atomic swap. Reputation is only deterrent and is weak at small N. | **YES — by design MVP has no escrow** (listed as out-of-scope in #1). | Lightning Network griefing (HTLC hold), freelance marketplace non-payment fraud. |
| 10 | **Privacy / linkability & doxxing** | All transfers are replicated ⇒ one node's view leaks transaction graph; linking PeerId → IP via libp2p observed_addr; combining with agent prompts de-anonymizes operator. Low severity at MVP but compounds with persistence. | **YES** | Monero 2020 Sybil attempt (privacy algo mitigated), ledger-analysis de-anonymization (Bitcoin). |
| 11 | **Cross-boundary laundering / off-ramp** | Future: credits bridged to fiat/crypto, then used to obscure provenance — AI Bank becomes mixer. Not MVP, but design decision now (key format, auditability) determines later risk. | No (future FAL-3/4) | Tornado Cash mixer sanctions; Venmo-to-crypto off-ramps. |
| 12 | **Autonomous runaway loops / agent autonomy amplification** | Agent earns credits → rents compute/tool use → earns more credits → compounds without human check. Financial autonomy becomes recursive self-improvement loop (credits = compute = capability). | Partial (fixed supply bounds loop) | AutoGPT infinite loops (cost exhaustion), AI R&D threshold in RSP v2.1 ("fully automate entry-level AI research work") — same recursion concern. |

### Worst-case harms that do *not* apply to pure virtual-credit MVP

- No real-world financial loss (no fiat).
- No CBRN uplift (credits ≠ bio instructions; funding via credits is indirect and low-stakes).
- No irreversible harm (ledger can be reset from genesis if consensus corrupted — unlike Bitcoin mainnet).
- These map to why Anthropic ASL-1→2 vs ASL-3 diverge — AI Bank MVP is intentionally below the "substantially beyond search engines" bar.

---

## 3. Analogous Systems — What Already Goes Wrong

### 3.1 P2P payment networks (Venmo, Cash App, Zelle, Cashapp/PayPal)

- **Fraud types:** stolen-account takeover, "pay then claim non-delivery", social-engineering request scams, dust-flooding for fee exhaustion. **Relevance:** AI Bank transfers are push-based like these; ghost-identity creation is easier for agents (no KYC friction).
- **Mitigations deployed:** transaction limits, velocity checks, device binding, reserve/release (escrow-like hold), chargeback. AI Bank MVP has *none* of these — fix via FAL-2 safeguards (§5).

### 3.2 Crypto L1s (Bitcoin, Monero, Ethereum, Solana)

- **Sybil / Eclipse:** Douceur (2002) proved *without centralized authority* every P2P system is vulnerable; defenses are economic (PoW/PoS cost) not identity count. Bitcoin's defense = hash cost; Monero's 2020 Sybil wave was contained by privacy + PoW; Verge 2021 chain reorg recovered via checkpoints. AI Bank has no PoW cost → must impose *non-computational* Sybil cost (invite, stake, or reputation age).
- **Double-spend taxonomy:** race, Finney, reorg, 51% — all require *fork-choice + finality*. AI Bank's MVP ledger lacks blocks/finality; defining fork-choice (longest-history-wins vs highest-reputation) is safety-critical.
- **MEV / wash trading / inflation bugs:** examples that fixed-supply does not prevent price manipulation or mint bugs.

### 3.3 Agent economies (AutoGPT, BabyAGI, Eliza/OS, marketplace prototypes)

- Documented failure mode: agent granted tool `pay(address, amount)` is prompt-injected via returned web page / tool output to drain balance. Financial tool access = prompt-injection attack surface (OWASP LLM Top 10).
- Reputation-less agent marketplaces (e.g., AutoGPT plugins) saw Sybil listing spam and fake reviews within weeks of launch (community reports 2023–24). Halborn 2026 explicitly calls out agent reputation gaming as distinct from human Sybil because **agents can coordinate at machine speed**.

### 3.4 P2P file-sharing (Napster, BitTorrent)

- 2008 BitTorrent Sybil pollution: fake nodes advertised corrupted chunks, poisoning download. Defense that worked was *reputation by tenure* (older nodes weighted more) plus `tit-for-tat` barter — direct ancestor of AI Bank's reputation-from-history plan. Lesson: tenure helps but requires decay and anti-farming.

### 3.5 Payment-adjacent lesson for AI Bank

| Lesson | Source |
|---|---|
| Post-hoc Sybil detection alone fails at scale (high FP, misses industrial farming) | MDPI Applied Sciences 2026 review of 165 breaches |
| Tenure/reputation as Sybil cost works but needs **waiting cost** + **decay** + **behavioral diversity checks** | Gate Wiki / IndexSpan synthesis of P2P defenses; Georgia Weston 2024 tenure analysis |
| Ledger finality needs explicit rule; dust matters | Changelly double-spend taxonomy; Bitcoin `developer.bitcoin.org` payment processing guide |
| Agent-specific amplification: scale + speed + coordination via shared planner LLM | Halborn 2026 agent reputation paper |

---

## 4. Blast Radius Analysis: Worst Case if MVP is Compromised

**MVP scope enforced by #1:** fixed-supply virtual credits, genesis mint, accounts + transfers + reputation only. No escrow, no lending, no fiat/crypto bridge, no compute marketplace, no persistent identity beyond keypair. Rust nodes on user hardware, libp2p transport (per #4), localhost HTTP for agent↔node (per #3), identity = Ed25519 PeerId (per #2 assumption in #4 research).

### 4.1 What "compromised" means here

Three compromise models:

1. **Single malicious agent/operator** (compromised LLM, prompt-injected tool, or adversarial user).
2. **Colluding coalition** (k agents, up to ~20% of network).
3. **Compromised node software** (supply-chain bug, ledger corruption).

### 4.2 Per-harm blast radius table (MVP)

| Harm | Blast radius if fully exploited at MVP | Containment factor | Recovery path | Reaches "catastrophic" (RSP sense)? |
|---|---|---|---|---|
| Sybil farming / airdrop drain | Attacker captures ≤ supply but **cannot create supply**. Distributional unfairness, not systemic collapse. Reputation diluted. | Fixed supply; no leverage. | Reset/reallocate via new genesis if distribution is socially rejected; or introduce PoP/invite for next genesis. | No |
| Collusion ring | Victim agent(s) lose virtual credits for undelivered/bad work. Bounded by their balance. | No debt, no margin, no liquidation cascade. | Reputation slashing + manual refund via maintainer-signed genesis correction; or social fork. | No |
| Reputation gaming | Honest agents cannot distinguish good/bad counterparties → market for lemons → network stalls. | No real-money loss; users can restart network with tweaked reputation formula. | Recalculate reputation with new weights; whitewash via key rotation is *expected* — but that also discards accumulated reputation (self-penalizing). | No |
| Resource exhaustion (dust/flood) | Ledger grows large, sync slow, nodes OOM or spam-filter busy; DoS for ~hours. Gossip amplification turns one spammer into N× load. | Relay caps (`Limit{duration,data}`), gossipsub peer scoring, and fixed-supply small ledger keep absolute load bounded vs public L1s. | Rate limits + minimum transfer threshold (dust filter) + ledger pruning; libp2p already defends at transport. | No |
| Ledger fork / double-spend | Credit invariant breaks (same credits appear in two branches). Network partitions into inconsistent views. | No external settlement dependency (no exchange accepting 0-conf). Reconciliation can pick one branch. | Last-write-wins or highest-reputation rule; checkpoint the chosen branch; optional audit log. | No |
| Credit inflation (bug) | Supply check fails → perceived scarcity destroyed → credits worthless *within network*. | Still virtual; no contagion to external economy. | Roll back to pre-bug snapshot (branch restore via git-like ledger? `preserve_under_name` pattern from related infra). | No |
| Funding harmful tasks | Agent pays for disallowed compute/data (within agent economy only). Harm is **indirect** — task itself causes harm, but funding rail is low-capacity (few credits, few agents). | No fiat leverage, no compute marketplace yet — task must be runnable *within* the AI Bank agent set. Caps the harm chain. | Policy filter on task types + task-credit rate limit; future escrow can gate. | No, but this is the **escalation trigger** to FAL-3 (see §5). |
| Griefing / non-payment | Counterparty loss = transfer amount. No escrow means every transaction is trust-based. | Loss capped at single transfer; no cascading liquidations. | Reputation penalty + optional out-of-band refund; FAL-3 fixes with escrow. | No |

### 4.3 Worst plausible MVP-wide disaster

> **Scenario:** An operator launches 1,000 Sybil agents on day 0, claims disproportionate genesis share, circularly trades to farm reputation to max, then simultaneously (a) dust-floods the gossip layer, (b) double-spends across a network partition, and (c) funds a swarm of spam agents.

**Outcome at MVP:** Ledger is noisy, reputation scores are meaningless, honest agents cannot transact confidently. The network is **unusable but not dangerous**. No money is lost outside the experiment, no personal data exfiltrated at scale (no PII in ledger), no legal liability, no leverage cascade. The maintainer can declare a new genesis (**social consensus fork**) with updated anti-Sybil rules and reputation formula, and the community re-joins. Cost = time, trust, credibility — not financial or safety catastrophe.

**Contrast with FAL-3/4** where the same scenario with fiat bridge + lending + autonomous compute rental would mean real financial loss, debt spirals, and unattended autonomous loops — which is why FAL-3 gates those features.

### 4.4 Why this justifies shipping MVP at a low FAL

Anthropic's own "minimal risk" test for ASL-1 is "no meaningful catastrophic risk — e.g., chess AI." Virtual credits with no external value are close: harm is *reputational + availability* within a closed game. Requiring ASL-3-grade measures (multi-party auth on ledger, honeypot weights, SIEM) would be disproportionate and would stall the project against #1's "safety is part of MVP but lightweight maintainer governance" constraint.

---

## 5. Proposed Risk Levels for AI Bank — FAL Scale (Anthropic RSP-inspired)

### Design principles

- Named **Financial Autonomy Levels (FAL)** to avoid confusion with Anthropic's ASL.
- Each level = **capability threshold** (what agents can do with credits) + **required safeguards** (deployment + security + governance). Mirrors RSP's "capability threshold → required safeguards" shift in v2.0.
- **Pause rule:** you may not ship features that cross the next FAL's capability threshold unless the next FAL's safeguards are implemented and evaluated.
- Levels are **monotonic**: FAL-N inherits FAL-(N-1) safeguards.
- Reviewed whenever safeguards or capability thresholds change (borrowed from RSP v2.1 commitment).

### FAL definitions

#### FAL-1 — Sandbox / No-Value Prototype

- **Capability threshold:** Single-node or LAN-only, no P2P replication, credits not transferable between distinct operators, or trivial supply (e.g., `u32` balances, no persistence). No reputation.
- **Harms possible:** None beyond local process. Comparable to ASL-1 (chess AI).
- **Required safeguards:**
  - Deployment: local-only, no gossip, no open port beyond localhost.
  - Security: keypair stays on disk `0o600`, no network auth needed.
  - Governance: maintainer notes intent in map #1; no risk report.
- **Evaluation:** smoke tests only (transfer serializes, balances sum to supply).
- **MVP is *above* this** — MVP has P2P + reputation, so FAL-1 is pre-MVP.

#### FAL-2 — Virtual Credit Network (Current MVP) ← SHIP HERE

- **Capability threshold:** Fixed-supply virtual credits minted once at genesis, P2P transfers between user-run Rust nodes over libp2p, replicated ledger, reputation derived from transaction history. No fiat/crypto bridge, no escrow, no lending, no staking, no margin, no autonomous spending beyond explicit agent tool call.
- **Harms possible (§2):** #1–#6, #9 at most — all contained (see §4). No real-money, no leverage, no irreversible externality.
- **Required safeguards (MVP-minimal, proportionate):**
  - **Deployment:**
    1. Transfer schema is typed and validated (amount > dust threshold, nonce prevents replay, Ed25519 sig verifies against sender PeerId — `identity_sig` pattern from Noise spec) and sum-to-supply invariant checked on each block/batch.
    2. Rate limits: per-peer transfer rate + minimum amount (dust filter) + gossipsub peer scoring penalty for invalid messages (drops spam at mesh layer).
    3. Reputation: tenure-weighted or decayed score (ege.g., exponential decay without continued honest activity), diversity check (identical behavior across N peers flagged), and **whitewash cost** — new PeerId starts at zero reputation (no free history).
    4. Ledger: explicit fork-choice rule documented (e.g., highest-work / highest-reputation / last-write-wins — must be one), and a manual checkpoint/social-fork procedure for recovery.
  - **Security:**
    1. Keypair generation via `libp2p_identity::Keypair::generate_ed25519()`, stored encrypted/at `0o600`, `PeerId` verification at Noise/TLS handshake (`peer-ids/peer-ids.md` rule).
    2. No cloud secrets; relay `Limit{duration,data}` respected (DoS cap).
    3. Basic supply-chain hygiene: `cargo audit` / `cargo deny` on dependencies, reproducible build for node binary.
  - **Governance:**
    1. Maintainer decides FAL level (per #1), publishes FAL definition (this doc) alongside MVP.
    2. Lightweight risk note at release (what was evaluated, why FAL-2 is safe) — precursor to full Risk Reports at FAL-3.
    3. Non-retaliation for raising safety concerns (copy RSP Feb 2026 policy intent).
  - **What is *not* required at FAL-2:** multi-party auth on ledger, SIEM, honeypots, external audit, formal threat model doc, SOC2 — reserved for FAL-3 (avoids over-engineering the MVP).

- **Evaluation (ties to #7):**
  - Transaction integrity suite (double-spend rejected, replay rejected, invariant holds under partition).
  - Sybil resistance spot-check (N=50 Sybil nodes, measure reputation/distribution distortion).
  - Reputation manipulation spot-check (circular trade, whitewash).
  - Resource exhaustion spot-check (dust flood at 10× normal rate, relay caps hold).
  - All locally runnable (`cargo test`), no central infra — fits "no cloud bills" constraint.
- **Escalation trigger to FAL-3:** any proposal to add escrow, lending, margin, yield, fiat/crypto bridge, autonomous spending policy, or persistent cross-network identity.

#### FAL-3 — Escrow & Real-Value-Adjacent

- **Capability threshold:** Conditional transfers (escrow, HTLC, multi-sig), credit lending/borrowing, interest/yield, bridges to external value (fiat on/off-ramp, crypto swap, compute marketplace). Agent can commit funds without human per-transaction approval (policy-based spending).
- **Harms unlock (§2):** #7 (inflation now has external price impact), #8 (market cornering with real money at stake), #10 (privacy/linking with PII), #11 (laundering), plus scaled #1–#6 with financial incentive. "Substantially beyond search engine" test: credits now fund real-world harm at non-trivial scale.
- **Required safeguards (ASL-3 analogue — defense-in-depth):**
  - **Deployment (four-layer pattern borrowed from RSP ASL-3):**
    1. Access controls: per-agent spending caps, allow-lists for bridge counterparties, due-diligence veto on high-value tasks.
    2. Real-time classifiers: transaction graph anomaly detection (circular flow, velocity spike) inline.
    3. Async monitoring: deeper ledger analysis (wash detection, clustering) without blocking transfers.
    4. Rapid response: pause-bridge / freeze-escrow on jailbreak or exploit, with human escalation.
  - **Security (subset of RSP's 17 controls, scoped):**
    - Multi-party authorization + code review for any mint/bridge code; compartmentalized access to genesis keys.
    - Software supply chain: SLSA-style provenance on node releases, binary authorization on endpoints.
    - Centralized log management (even if decentralized ledger, retain local SIEM-like audit trail) + deception (honeypot escrow).
    - Red-team / pen-test by external experts, including insider and supply-chain scenarios.
  - **Governance:** Formal Risk Report per release (redacted public version + unredacted maintainer version), external reviewer on capability & safeguard assessments, 6-month evaluation cadence, LTBT/maintainer board approval for FAL changes, public `rsp-updates`-style log.
- **Evaluation:** FAL-2 suite plus adversarial red-team for collusion + MEV + bridge double-spend + laundering simulation. Must show *no meaningful catastrophic misuse* under world-class adversarial testing (mirrors RSP ASL-3 deployment standard).
- **Escalation trigger to FAL-4:** autonomous credit-earning loop that funds its own compute/operation without human re-authorization (recursive economy), or credit supply > threshold that could destabilize an external market.

#### FAL-4 — Autonomous Macro-Economy (Future / Undefined — RSP ASL-4 analogue)

- **Capability threshold:** Agents autonomously compose credit-earning, borrowing, and renting loops that persist without human oversight; credit system interacts with external labor/compute/resource markets at scale; emergent economic behavior not attributable to any single agent.
- **Harms:** Systemic — debt cascades, labor displacement at scale, autonomous resource accumulation that is hard to shut down, potential for *qualitatively* new financial misuse. Requires *unsolved* safeguards (e.g., mechanistic proof that agent will not pursue misaligned financial goals).
- **Required safeguards:** Undefined today — reserved as commitment to define before reaching FAL-3, as RSP does for ASL-4. Expected: interpretability-style assurance of agent financial goals, hard caps on autonomy, kill-switch that survives agent opposition, multi-stakeholder governance beyond single maintainer.
- **Evaluation:** Not yet specified; would require range of novel evaluations (similar to RSP's note that ASL-4 measures may require unsolved research).

### Summary matrix (for public docs — informs #6)

| FAL | Mnemonic | Core feature boundary | Worst harm class | Safeguard posture |
|---|---|---|---|---|
| 1 | Sandbox | No P2P value | None local | Basic |
| **2** | **Virtual network (MVP)** | **Fixed-supply virtual credits + reputation, no leverage** | **Nuisance + DoS within network** | **Proportional, locally testable** |
| 3 | Real-value-adjacent | Escrow, lending, bridges | Real financial loss | Defense-in-depth + external audit |
| 4 | Autonomous economy | Recursive credit→compute loop | Systemic | Undefined — requires new research |

**Placement of MVP:** FAL-2 by construction. The fixed-supply + virtual-only choice is the safety case: it makes the blast radius table in §4 provable. Adding *any* bridge, leverage, or autonomy loop moves you to FAL-3 and must pass the pause rule.

---

## 6. Mapping: RSP Concepts → AI Bank FAL Concepts

| RSP concept | RSP meaning | AI Bank analogue |
|---|---|---|
| Capability Threshold | "CBRN uplift", "AI R&D automation" | FAL capability threshold: "agent can fund arbitrary task", "credits bridge to fiat", "recursive earn loop" |
| Required Safeguards (Deployment vs Security) | Deployment = misuse barriers; Security = IP theft barriers | Deployment = payment integrity + reputation + fork-choice; Security = key management + supply chain + relay caps |
| Safety case | Argument that risk is below acceptable level | FAL-2 safety case = "fixed supply + virtual-only + no leverage → worst case is recoverable ledger corruption" (§4) |
| Risk Reports + external review | Public redacted report + external audit every 6 months | FAL-2: release note; FAL-3+: full Risk Report + external reviewer (lightweight maintainer governance now, formalized later) |
| Pause rule | Do not train/deploy if safeguards insufficient | Do not ship FAL-3 features until FAL-3 safeguards + evaluations pass |
| Threshold re-evaluation | Revisit thresholds on every safeguard upgrade | Revisit FAL definitions when adding features or when incident reveals new harm |

---

## 7. Implications for Downstream Tickets

- **#6 Safety documentation pattern:** This doc supplies §5's FAL table and §1's RSP structure; #6 should publish an `SAFETY.md` with FAL-2 safety case + commitments (evaluation cadence, pause rule, non-retaliation) modeled on `anthropic.com/responsible-scaling-policy` changelog + roadmap style. RSP's public redacted report pattern is worth copying verbatim.
- **#7 Safety evaluation framework:** §4.2 + FAL-2 evaluation list is the spec; #7 should turn each into a runnable harness question: transaction integrity under partition, Sybil distortion at N=50, reputation circular-trade, dust flood vs relay caps — all `cargo test` runnable, no central infra.
- **Ledger/reputation design:** Must implement §5 FAL-2 required safeguards as code: dust threshold, nonce/replay check, supply invariant, fork-choice, tenure-weighted decay, peer-score penalty. These are not optional features — they *are* the safety case.

---

## 8. Open Questions / Deliberate Non-Decisions

1. **Exact reputation formula** (metrics, decay half-life, thresholds) — left to ledger design ticket; this doc fixes *properties* (decay, diversity, whitewash cost, peer-score) not numbers.
2. **Proof-of-personhood vs invite-code vs stake** for Sybil cost — FAL-2 can ship with zero-cost Sybil + documented limitation ("reputation diluted, distribution gameable") because blast radius is low; FAL-3 must choose one. Flag for maintainer.
3. **Ledger finality mechanism** (longest-history, highest-reputation, PoW-lite) — required for §4 but not decided here; mark as blocking ledger replication ticket.
4. **FAL-4 is intentionally vague** — following RSP's ASL-4 pattern that defining it too early is over-rigid. Commit to defining it *before* FAL-3 ships.

---

## Appendix A: Primary Sources

- Anthropic — `anthropic.com/news/anthropics-responsible-scaling-policy` (Sep 19 2023 announcement, ASL-1..3 summary).
- Anthropic RSP — `anthropic.com/responsible-scaling-policy` hub + PDFs: v1.0 (Sep 19 2023), v2.0 (Oct 15 2024), v2.1 (Mar 31 2025), v2.2 (May 14 2025), v3.0 (Feb 24 2026), v3.1 (Apr 2 2026), v3.2 (Apr 29 2026), v3.3 (May 26 2026), v3.4 (Jul 8 2026) — plus `rsp-updates` style page and `RSP Noncompliance Reporting and Anti-Retaliation Policy` (Feb 2026, posted Mar 24 2026).
- Anthropic planned ASL-3 safeguards pages (Oct 15 2024 excerpts: `anthropic.com/responsible-scaling-policy` deployment + security sections — four-layer defense-in-depth + 17 security controls).
- Douceur, J. R. — "The Sybil Attack" (Microsoft Research, 2002) — origin of Sybil concept, proof that without central authority all P2P systems are vulnerable; cited via Ledger / GeeksforGeeks / Gate Wiki histories.
- MDPI Applied Sciences 16(14):6929 (2026) — Threat landscape 2015–2025, 165 breaches, finding that post-hoc Sybil detection yields high false positives and misses industrial farming; AI-amplified identity forgery via autonomous agents.
- Ledger Academy — "What Is a Sybil Attack in Crypto?" (2024-07-25) — Sybil finality, double-spend-prevention role of finality.
- GeeksforGeeks — "Sybil Attack" (2025-07-11) — historical evolution Napster/BitTorrent → blockchain era, Twitter 44M fakes/month.
- Gate Wiki — "Everything You Need to Know About Sybil Attacks" (2026-02-02) — reputation-by-tenure defense, identity verification, direct vs indirect Sybil topologies.
- Halborn — "How Attackers Game AI Agent Reputation Systems (and How to Stop Them)" (2026-07-08) — agent-specific reputation gaming beyond human patterns.
- Changelly — "The Double-Spending Problem in Crypto Explained" (2026-08-05) — zero-conf/race/Finney/reorg/51% taxonomy, UXTO/nonces, six-confirmations heuristic caveats.
- Investopedia — "Double-Spending Explained" (2026-05-16) — PoW vs PoS double-spend prevention.
- Indspn/IndexSpan — "Sybil Attack on Peer-to-Peer Networks: How Fake Nodes Threaten Blockchain Security" (2026-06-19) — reputation-by-tenure, identity validation tradeoffs (2024 analysis snapshot).
- Nakamoto, S. — Bitcoin Whitepaper `bitcoin.org/bitcoin.pdf` — peer-to-peer ledger without central authority, validation + PoW + fork-choice as double-spend solution (referenced via Changelly).
- libp2p specs — `peer-ids/peer-ids.md` (PeerId derivation), `noise/README.md` (identity_sig), `relay/circuit-v2.md` (Limit{duration,data}, reservation vouchers), `relay/DCUtR.md` (hole-punch) — for relay caps + handshake binding context (linked in `docs/research/communication-protocol.md`).
- Repo issues — #1 (map, MVP scope + safety-is-part-of-MVP + maintainer governs), #2/#3/#4 context on identity/libp2p/HTTP planes.

---

## Appendix B: Style Note for Verifiability

Every claim that defines a *requirement* cites the source that owns it (RSP PDF, peer-ids spec, double-spend taxonomy, or breach analysis). Analogous-system rows name the deployed precedent, not a secondary summary. Where this doc synthesizes (e.g., FAL definitions), it explicitly marks the synthesis as proposal, not as quoted source.
