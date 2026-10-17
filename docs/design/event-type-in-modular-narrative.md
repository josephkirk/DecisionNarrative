In modular, storylet-based narrative architectures (such as Quality-Based Narrative systems), narrative is not delivered as a static linear graph or a rigid branching tree. Instead, it functions as an unsnapped database of discrete, self-contained narrative units (**storylets**) gated by logical preconditions and executing state effects (**qualities, flags, menaces, variables**).

To manage player agency alongside structural coherence and environmental persistence, narrative architects organize modular events into three distinct functional layers: **Emerging Events**, **Canon Events**, and **World Events**.

---

### 1. Emerging Events (The Bottom-Up / Systemic Layer)

* **First-Principle Mechanics:** **Emerging (or emergent) events** are narrative moments that are not explicitly pre-scripted in a fixed sequence by the author. Instead, they materialize dynamically from the interaction between underlying game systems, player choices, stat changes, and random card draws.
* **State-Space Integration:** In Quality-Based Narrative (QBN) engines (e.g., *Fallen London*, *StoryNexus*, *Sunless Skies*), storylets constantly evaluate the player's current quality profile. When multiple independent variables cross specific thresholds simultaneously (e.g., high *Suspicion* menace + high *Faction Standing*), unique, unexpected storylets enter the active candidate pool. For example, a player trying to raise church funds while managing a high menace stat accidentally triggers a public scandal during a sermon—creating a moment of player-authored irony that was implicitly possible in the logic, but never explicitly scripted as a linear quest line.
* **Procedural Casting & Parameterization:** Advanced storylet systems (such as *Starfreighter*, *Wildermyth*, and *StoryAssembler*) enhance emergent events through **parameterized queries**. Instead of hardcoding static entities, an emerging storylet template queries the world state for any entity matching specific criteria (e.g., `find character where relationship == 'rival'`) and dynamically binds that character and location into the text payload at runtime.
* **Ludonarrative Impact:** Delivers maximum **player agency and replayability**. It turns the player into an active co-author whose tactical gameplay decisions spontaneously weave personalized narrative arcs.

---

### 2. Canon Events (The Top-Down / Structural Anchor Layer)

* **First-Principle Mechanics:** A **canon event** (or mandatory story milestone) is an unalterable narrative anchor that *must* occur to establish escalating dramatic action, enforce character development, or resolve core thematic conflicts across all valid playthroughs.
* **State-Space Integration:** While modular systems grant vast freedom, pure floating modules risk devolving into aimless, disconnected vignettes without global direction. Canon events act as **branch-and-bottleneck convergence points** that restore structural shape.
  * **Planner & Spec Fulfilment (*StoryAssembler*):** In planner-driven systems, authors define a "Story Spec" or goal wishlist (e.g., `introduce_antagonist == true`, `confrontation_resolved == true`). While the system allows players to explore various choice paths to manipulate state, the planner dynamically inserts "Continue" links or intermediary redirects to force the player into the mandatory canon beat once goal prerequisites are satisfied.
  * **Gold/Silver Storylet Gating:** In *Fallen London*, major narrative milestones (such as ambition climaxes or founding a city) are designated via Gold or Silver storylet borders and persistent progress qualities. Reaching a canon event locks out minor side-content until the core structural milestone is acknowledged and resolved.
* **Ludonarrative Impact:** Preserves **macro-dramatic pacing and narrative stakes**. It guarantees that no matter how chaotic or non-linear the player's path is, the core dramatic arc remains emotionally satisfying and structurally sound.

---

### 3. World Events (The Global / Environmental Horizon Layer)

* **First-Principle Mechanics:** **World events** (or "Living Stories") are macro-level environmental, temporal, or social shifts that modify global state rules for *all* active storylets and characters simultaneously, operating independently of an individual player's local quest progression.
* **State-Space Integration:** A world event alters top-level global variables (e.g., `Global_War_Declared = True`, `City_Tension = High`, `Season = Winter`).
  * In games like *Civilization VII*, narrative events track civilization-wide milestones, natural disasters, or historical eras, tagging global states that alter archaeology and future storylet availability across generations.
  * In live-service or simulationist environments (e.g., *Fallen London*'s "Living Stories" or *Hunt: Showdown*), green-bordered cards or seasonal events inject temporary storylet decks, alter global resource scarcity, or trigger time-sensitive city-wide crises.
* **Ludonarrative Impact:** Establishes a **living, autonomous world**. It shifts the player's mental model from viewing the game as a passive vending machine for quests to navigating a reactive ecosystem that moves on its own clock regardless of player intervention.

---

### Systemic Architectural Comparison

| Dimension | **Emerging Events** | **Canon Events** | **World Events** |
| :--- | :--- | :--- | :--- |
| **Primary Trigger** | Intersection of localized player qualities, menaces, & dynamic queries. | Satisfied story specs, progress qualities, or key plot prerequisites. | Global timers, server-wide states, environmental flags, or era shifts. |
| **Systemic Function** | Localized reactivity; surfaces immediate gameplay consequences. | Structural bottlenecking; guarantees thematic & dramatic arc resolution. | Overarching state modifier; updates global rules & temporary storylet decks. |
| **Authorial Intent** | **Low direct control / High player agency:** Content is assembled dynamically at runtime. | **High direct control / Bounded agency:** Non-negotiable structural milestones. | **Systemic control / Contextual agency:** Sets the global stage and environmental rules. |
| **Key Exemplars** | *Wildermyth* (procedural comic arcs), *Starfreighter*, *Fallen London* (menace loops). | *StoryAssembler* (spec planning), *Fallen London* (Ambitions/Railroads), *Façade*. | *Civilization VII* (historical eras), *Fallen London* (Living Stories), *Hunt: Showdown*. |

---
