// Weir's Crossing: viability prototype for docs/design/proposal-human-first-nonlinear-narrative.md
//
// This models the proposal's three non-linear axes on a tiny authored world, then explores it
// with seeded random players. It is not engine code; it checks whether the proposed contracts
// (participant-scoped knowledge with provenance, a fixed authored past told in any order,
// co-presence witnessing, NPC agency on a clock, and coverage-gap detection) behave as claimed.
//
// Run: bun examples/proposal-weirs-crossing/explore.ts [runs]

type Loc = "lighthouse" | "harbor" | "inn" | "customs" | "rock";
type KnowKind = "evidence" | "witnessed" | "told" | "rumor" | "recollection" | "testimony" | "inference";

const SLOTS = ["D1 morning", "D1 evening", "D2 morning", "D2 evening", "D3 morning", "D3 evening"];
const ACTIONS_PER_SLOT = 2;
const TIDE_RULE = process.env.NO_TIDE_RULE ? false : true;
const LOW_TIDE_SLOTS = new Set([1, 4]);
const ADJ: Record<Loc, Loc[]> = {
  lighthouse: ["harbor", "rock"],
  harbor: ["lighthouse", "inn", "customs"],
  inn: ["harbor", "customs"],
  customs: ["harbor", "inn"],
  rock: ["lighthouse"],
};

// The authored past (fabula). Fixed canon: recollections may reveal it in any order, never change it.
const CANON_PAST: Record<string, string> = {
  P1: "22:00 lighthouse - Ilse lights the lamp and logs the storm",
  P2: "23:00 harbor - an unlit boat lands crates; a customs lantern on the shingle",
  P3: "23:30 lighthouse - the lamp is doused; two cans of lamp oil signed out to 'B.'",
  P4: "00:30 sea - Ilse rows for the outer rock to warn ships off the reef",
};

interface Knowledge { kind: KnowKind; sourceEvent: number }
interface Actor { id: string; loc: Loc; player: boolean; knows: Map<string, Knowledge> }
interface Event { id: number; slot: number; actor: string; action: string; causedBy: number[]; loc: Loc }

interface Storylet {
  id: string;
  at: Loc | "anywhere";
  playableBy?: string[];          // participant ids; default: any player
  needs?: string[];               // all known by the acting participant
  needsAny?: string[];            // at least one known
  lacks?: string[];               // none known
  present?: string[];             // actors that must be co-located (and not gone)
  absent?: string[];              // actors that must NOT be co-located
  worldNot?: string[];            // world props that must be unset
  minPlayersPresent?: number;
  once: "participant" | "world" | "never";
  learn?: [string, KnowKind][];
  revealPast?: string[];
  set?: string[];
  visible?: boolean;              // co-located actors witness the outcome facts
  rosaHears?: string[];           // facts Rosa learns as rumour ("told") if played at the inn
  fallback?: boolean;             // generic filler, not story progress
  origin?: "human" | "llm-draft-approved";
}

const BASE_STORYLETS: Storylet[] = [
  { id: "lamp_soot", at: "lighthouse", once: "participant", learn: [["light_doused", "evidence"]] },
  { id: "ilse_logbook", at: "lighthouse", once: "participant", revealPast: ["P1"], learn: [["where.rock", "evidence"]] },
  { id: "harbor_master_tides", at: "harbor", once: "participant", learn: [["tide.table", "told"]] },
  // Teodor plays his own memory when he is a participant...
  { id: "teodor_lantern_memory", at: "harbor", playableBy: ["teodor"], once: "participant",
    revealPast: ["P2"], learn: [["crates_landed", "recollection"], ["benedek.suspected", "recollection"], ["belief.ilse_neglected", "testimony"]] },
  // ...or Mara presses the NPC Teodor for it, once she has a reason to.
  { id: "press_teodor", at: "harbor", playableBy: ["mara"], present: ["teodor"], needs: ["light_doused"], once: "participant",
    revealPast: ["P2"], learn: [["crates_landed", "testimony"], ["benedek.suspected", "testimony"], ["belief.ilse_neglected", "testimony"]] },
  { id: "customs_ledger", at: "customs", absent: ["benedek"], needs: ["crates_landed"], once: "participant",
    revealPast: ["P3"], learn: [["benedek.identified", "evidence"]] },
  { id: "ask_rosa_openly", at: "inn", once: "participant", learn: [["benedek.suspected", "rumor"]], rosaHears: ["light_doused", "benedek.suspected"] },
  { id: "listen_quietly", at: "inn", once: "participant", learn: [["benedek.suspected", "rumor"]] },
  { id: "piece_together", at: "anywhere", needs: ["benedek.suspected", "light_doused", "crates_landed"], lacks: ["benedek.identified"],
    once: "participant", learn: [["benedek.identified", "inference"]] },
  { id: "confront_benedek", at: "anywhere", present: ["benedek"], needs: ["benedek.identified"], worldNot: ["benedek.delayed"],
    once: "world", set: ["benedek.delayed"], visible: true, learn: [["benedek.confessed", "witnessed"]] },
  { id: "find_ilse", at: "rock", once: "world", revealPast: ["P4"], set: ["ilse.rescued"], visible: true, learn: [["ilse.alive", "witnessed"]] },
  { id: "watch_the_water", at: "anywhere", once: "never", fallback: true },
];

// Drafted by an LLM from the coverage report, edited and approved by the designer (v2 only).
const BLUFF: Storylet = {
  id: "bluff_benedek", at: "anywhere", present: ["benedek"], needs: ["benedek.suspected"], lacks: ["benedek.identified"],
  needsAny: ["light_doused", "crates_landed"], worldNot: ["benedek.delayed"], once: "world",
  set: ["benedek.delayed"], visible: true, learn: [["benedek.identified", "witnessed"], ["benedek.confessed", "witnessed"]],
  origin: "llm-draft-approved",
};

// ---------- deterministic RNG ----------
function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

interface RunResult {
  ending: string;
  revealOrder: string[];
  m2Route?: string;
  holeMoments: number; holeFilled: number;
  fallbackOnlyActions: number; totalActions: number;
  tipOff: boolean; tipOffChain?: string; strandings: number;
  violations: string[];
}

type Policy = "wanderer" | "investigator";
function weighted<T extends { w: number }>(options: T[], rand: () => number): T {
  const total = options.reduce((t, o) => t + o.w, 0);
  let r = rand() * total;
  for (const o of options) { r -= o.w; if (r <= 0) return o; }
  return options[options.length - 1];
}

function run(seed: number, coop: boolean, storylets: Storylet[], policy: Policy): RunResult {
  const rand = rng(seed);
  const actors: Record<string, Actor> = {
    mara: { id: "mara", loc: "inn", player: true, knows: new Map() },
    teodor: { id: "teodor", loc: "harbor", player: coop, knows: new Map() },
    benedek: { id: "benedek", loc: "customs", player: false, knows: new Map() },
    rosa: { id: "rosa", loc: "inn", player: false, knows: new Map() },
  };
  const world = new Set<string>();
  const played = new Set<string>();
  const events: Event[] = [];
  const revealOrder: string[] = [];
  const res: RunResult = { ending: "", revealOrder, holeMoments: 0, holeFilled: 0, fallbackOnlyActions: 0, totalActions: 0, tipOff: false, strandings: 0, violations: [] };

  const emit = (slot: number, actor: string, action: string, loc: Loc, causedBy: number[] = []) => {
    const e = { id: events.length, slot, actor, action, causedBy, loc };
    events.push(e);
    return e.id;
  };
  const learn = (a: Actor, fact: string, kind: KnowKind, ev: number) => { if (!a.knows.has(fact)) a.knows.set(fact, { kind, sourceEvent: ev }); };
  const gone = (id: string) => id === "benedek" && world.has("benedek.gone");
  const coLocated = (loc: Loc) => Object.values(actors).filter((x) => x.loc === loc && !gone(x.id));

  // Seed Rosa with the ferryman's rumour (authored initial knowledge, with provenance).
  learn(actors.rosa, "benedek.suspected", "rumor", emit(0, "ferryman", "told_rosa", "inn"));

  const eligible = (s: Storylet, a: Actor, slot: number): boolean => {
    if (s.at !== "anywhere" && s.at !== a.loc) return false;
    if (s.playableBy && !s.playableBy.includes(a.id)) return false;
    if (s.once === "participant" && played.has(`${s.id}:${a.id}`)) return false;
    if (s.once === "world" && played.has(s.id)) return false;
    if (s.needs?.some((f) => !a.knows.has(f))) return false;
    if (s.needsAny && !s.needsAny.some((f) => a.knows.has(f))) return false;
    if (s.lacks?.some((f) => a.knows.has(f))) return false;
    if (s.worldNot?.some((p) => world.has(p))) return false;
    const here = coLocated(a.loc).map((x) => x.id);
    if (s.present?.some((id) => !here.includes(id) || gone(id))) return false;
    if (s.absent?.some((id) => here.includes(id))) return false;
    if (s.minPlayersPresent && coLocated(a.loc).filter((x) => x.player).length < s.minPlayersPresent) return false;
    void slot;
    return true;
  };

  const play = (s: Storylet, a: Actor, slot: number) => {
    played.add(s.once === "world" ? s.id : `${s.id}:${a.id}`);
    const ev = emit(slot, a.id, s.id, a.loc);
    for (const p of s.revealPast ?? []) {
      if (!(p in CANON_PAST)) res.violations.push(`reveal of non-canon past ${p}`);
      if (!revealOrder.includes(p)) revealOrder.push(p);
      learn(a, `past.${p}`, "recollection", ev);
    }
    const knewBefore = new Set(a.knows.keys());
    for (const [f, k] of s.learn ?? []) {
      learn(a, f, k, ev);
      if (s.visible) for (const w of coLocated(a.loc)) if (w !== a) learn(w, f, "witnessed", ev);
    }
    for (const p of s.set ?? []) world.add(p);
    if (s.rosaHears && a.loc === "inn") for (const f of s.rosaHears) if (knewBefore.has(f)) learn(actors.rosa, `heard:${a.id}:${f}`, "told", ev);
    if (a.knows.get("benedek.identified")?.sourceEvent === ev) res.m2Route ??= s.id;
  };

  // Traversal persona "investigator": a goal-directed QA bot, not a model of real players.
  const RANK = ["find_ilse", "confront_benedek", "bluff_benedek"];
  const progressAt = (a: Actor, loc: Loc, slot: number) => {
    const real = a.loc; a.loc = loc;
    const ok = storylets.some((s) => !s.fallback && s.at === loc && eligible(s, a, slot));
    a.loc = real; return ok;
  };
  const stepToward = (from: Loc, goal: (l: Loc) => boolean): Loc | undefined => {
    const prev = new Map<Loc, Loc>([[from, from]]); const q: Loc[] = [from];
    while (q.length) {
      const cur = q.shift()!;
      if (cur !== from && goal(cur)) { let c = cur; while (prev.get(c) !== from) c = prev.get(c)!; return c; }
      for (const n of ADJ[cur]) if (n !== "rock" && !prev.has(n)) { prev.set(n, cur); q.push(n); }
    }
    return undefined;
  };
  type Opt = { w: number; go: () => void; label: string; kind: "story" | "fallback" | "move" | "notes"; to?: Loc };
  const investigate = (a: Actor, slot: number, options: Opt[], partner?: Actor): Opt => {
    if (rand() < 0.15) return weighted(options, rand); // exploration noise so the bot covers more than one path
    const stories = options.filter((o) => o.kind === "story");
    const ranked = stories.find((o) => RANK.includes(o.label));
    if (ranked) return ranked;
    const rockReady = a.knows.has("where.rock") && a.knows.has("tide.table") && !world.has("ilse.rescued");
    if (stories.length && !(rockReady && (LOW_TIDE_SLOTS.has(slot) || LOW_TIDE_SLOTS.has(slot + 1)))) return stories[Math.floor(rand() * stories.length)];
    if (rockReady) {
      const toRock = options.find((o) => o.to === "rock");
      if (toRock) return toRock;
      if ((LOW_TIDE_SLOTS.has(slot) || LOW_TIDE_SLOTS.has(slot + 1)) && a.loc !== "lighthouse") {
        const n = stepToward(a.loc, (l) => l === "lighthouse");
        const mv = options.find((o) => o.to === n); if (mv) return mv;
      }
    }
    if (stories.length) return stories[Math.floor(rand() * stories.length)];
    if (partner && [...partner.knows.keys()].some((f) => !a.knows.has(f))) { const n = options.find((o) => o.kind === "notes"); if (n) return n; }
    const n = stepToward(a.loc, (l) => progressAt(a, l, slot));
    const mv = options.find((o) => o.to === n);
    return mv ?? options[Math.floor(rand() * options.length)];
  };

  for (let slot = 0; slot < SLOTS.length; slot++) {
    // World clock: the tide. Found by the invariant check on the first run (participants were left on
    // the rock after the causeway flooded); the author then added this rule.
    if (TIDE_RULE && !LOW_TIDE_SLOTS.has(slot)) for (const a of Object.values(actors)) if (a.loc === "rock") {
      emit(slot, "tide", `drives_${a.id}_back`, "rock"); a.loc = "lighthouse"; res.strandings++;
    }
    // World clock: Benedek's schedule (customs mornings, inn evenings).
    if (!gone("benedek")) actors.benedek.loc = slot % 2 === 0 ? "customs" : "inn";

    for (let step = 0; step < ACTIONS_PER_SLOT; step++) {
      // NPC agency: Rosa repeats what she heard to whoever shares the inn; Benedek reacts to being named.
      if (!gone("benedek") && actors.benedek.loc === "inn") {
        const heard = [...actors.rosa.knows.entries()].find(([f]) => f.startsWith("heard:") && f.endsWith(":benedek.suspected"));
        if (heard && !actors.benedek.knows.has("i.am.suspected")) {
          const ev = emit(slot, "rosa", "gossips_to_benedek", "inn", [heard[1].sourceEvent]);
          learn(actors.benedek, "i.am.suspected", "rumor", ev);
        }
      }
      const players = Object.values(actors).filter((x) => x.player).sort((x, y) => x.id.localeCompare(y.id));
      for (const a of players) {
        res.totalActions++;
        const options: { w: number; go: () => void; label: string; kind: "story" | "fallback" | "move" | "notes"; to?: Loc }[] = [];
        const stories = storylets.filter((s) => eligible(s, a, slot));
        const progress = stories.filter((s) => !s.fallback);
        if (progress.length === 0) res.fallbackOnlyActions++;
        for (const s of stories) options.push({ w: s.fallback ? 0.3 : 3, label: s.id, kind: s.fallback ? "fallback" : "story", go: () => play(s, a, slot) });
        for (const to of ADJ[a.loc]) {
          if (to === "rock" && !(LOW_TIDE_SLOTS.has(slot) && a.knows.has("tide.table"))) continue;
          options.push({ w: 1, label: `move:${to}`, kind: "move", to, go: () => { emit(slot, a.id, `move:${to}`, a.loc); a.loc = to; } });
        }
        if (a.loc === "rock" && !LOW_TIDE_SLOTS.has(slot)) res.violations.push("participant on rock at high tide");
        // Co-op: compare notes with a co-located player (knowledge moves only by an in-world act).
        const partner = players.find((p) => p !== a && p.loc === a.loc);
        if (partner) options.push({ w: 1.5, label: "compare_notes", kind: "notes", go: () => {
          const ev = emit(slot, a.id, "compare_notes", a.loc);
          for (const [f] of partner.knows) learn(a, f, "told", ev);
          for (const [f] of a.knows) learn(partner, f, "told", ev);
        } });
        // Coverage hole: the participant holds the intent (suspects Benedek, faces him) but no storylet affords it.
        const facingBenedek = !gone("benedek") && !world.has("benedek.delayed") && a.loc === actors.benedek.loc;
        if (facingBenedek && a.knows.has("benedek.suspected") && !a.knows.has("benedek.identified")) {
          res.holeMoments++;
          if (stories.some((s) => s.id === "bluff_benedek")) res.holeFilled++;
        }
        const pick = policy === "wanderer" ? weighted(options, rand) : investigate(a, slot, options, partner);
        pick.go();
      }
    }

    // End of slot: Benedek leaves early if tipped off, otherwise on the D2 evening ferry.
    if (!gone("benedek") && !world.has("benedek.delayed")) {
      const tipped = actors.benedek.knows.get("i.am.suspected");
      if (tipped || slot === 3) {
        const ev = emit(slot, "benedek", "takes_ferry", actors.benedek.loc, tipped ? [tipped.sourceEvent] : []);
        world.add("benedek.gone");
        if (tipped && slot < 3) {
          res.tipOff = true;
          const chain: string[] = [];
          let cur: Event | undefined = events[ev];
          while (cur) { chain.unshift(`${SLOTS[cur.slot]} ${cur.actor}:${cur.action}@${cur.loc}`); cur = cur.causedBy.length ? events[cur.causedBy[0]] : undefined; }
          res.tipOffChain = chain.join(" -> ");
        }
      }
    }
  }

  // Invariants: every belief has an in-world source event; nobody knows the past without a telling.
  for (const a of Object.values(actors)) for (const [f, k] of a.knows) {
    if (!events[k.sourceEvent]) res.violations.push(`${a.id} knows ${f} without a source event`);
    if (f.startsWith("past.") && !revealOrder.includes(f.slice(5))) res.violations.push(`${a.id} knows ${f} but it was never told`);
  }
  if (world.has("benedek.delayed") && events.find((e) => e.action === "takes_ferry")) res.violations.push("benedek both delayed and departed");

  if (process.env.TRACE) for (const e of events) console.log(`  #${e.id} ${SLOTS[e.slot]} ${e.actor}:${e.action}@${e.loc}${e.causedBy.length ? " <- #" + e.causedBy.join(",#") : ""}`);
  if (process.env.TRACE) console.log("  violations:", res.violations.slice(0, 5));
  const rescued = world.has("ilse.rescued");
  const justice = world.has("benedek.delayed");
  res.ending = rescued && justice ? "E1 Light Restored" : rescued ? "E2 The Keeper Comes Home" : justice ? "E3 A Ledger With a Name" : "E4 Low Water";
  return res;
}

function report(label: string, coop: boolean, storylets: Storylet[], runs: number, policy: Policy) {
  const endings = new Map<string, number>();
  const orders = new Map<string, number>();
  const m2 = new Map<string, number>();
  let hole = 0, filled = 0, fbo = 0, actions = 0, tip = 0, violations = 0;
  let sampleChain = "";
  for (let i = 0; i < runs; i++) {
    const r = run(1000 + i, coop, storylets, policy);
    endings.set(r.ending, (endings.get(r.ending) ?? 0) + 1);
    if (r.revealOrder.length >= 2) orders.set(r.revealOrder.join(">"), (orders.get(r.revealOrder.join(">")) ?? 0) + 1);
    if (r.m2Route) m2.set(r.m2Route, (m2.get(r.m2Route) ?? 0) + 1);
    hole += r.holeMoments; filled += r.holeFilled; fbo += r.fallbackOnlyActions; actions += r.totalActions;
    if (r.tipOff) { tip++; sampleChain ||= r.tipOffChain ?? ""; }
    violations += r.violations.length;
  }
  const pct = (n: number, d = runs) => `${((100 * n) / d).toFixed(1)}%`;
  console.log(`\n=== ${label} [${policy}] (${runs} seeded runs) ===`);
  console.log("endings:", [...endings].sort().map(([k, v]) => `${k}=${pct(v)}`).join(" | "));
  console.log("benedek identified via:", [...m2].sort((a, b) => b[1] - a[1]).map(([k, v]) => `${k}=${pct(v)}`).join(" | "));
  console.log(`distinct past-telling orders (>=2 reveals): ${orders.size}; top:`, [...orders].sort((a, b) => b[1] - a[1]).slice(0, 4).map(([k, v]) => `${k}=${pct(v)}`).join(" | "));
  console.log(`runs where gossip tipped Benedek off: ${pct(tip)}`);
  console.log(`coverage-hole moments (suspects + faces Benedek, no affordance): ${hole} total, ${filled} filled by bluff storylet`);
  console.log(`actions with only fallback content available: ${pct(fbo, actions)}`);
  console.log(`invariant violations: ${violations}`);
  if (sampleChain) console.log(`sample causal chain: ${sampleChain}`);
}

const runs = Number(process.argv[2] ?? 20000);
if (process.env.FINDE) { const [c, pol, end] = process.env.FINDE.split(":"); for (let i = 1000; i < 21000; i++) { const r = run(i, c === "coop", [...BASE_STORYLETS, BLUFF], pol as Policy); if (r.ending.startsWith(end)) { console.log(i); break; } } process.exit(0); }
if (process.env.FINDV) { for (let i = 1000; i < 1200; i++) { const r = run(i, process.env.FINDV === "coop", BASE_STORYLETS, "wanderer"); if (r.violations.length) { console.log(i, r.violations); break; } } process.exit(0); }
if (process.env.TRACE) { const [c, pol, seed] = process.env.TRACE.split(":"); console.log(run(Number(seed), c === "coop", [...BASE_STORYLETS, BLUFF], pol as Policy).ending); process.exit(0); }
for (const policy of ["wanderer", "investigator"] as Policy[]) {
  report("v1 single-player (Teodor is an NPC)", false, BASE_STORYLETS, runs, policy);
  report("v1 co-op (Mara + Teodor are players)", true, BASE_STORYLETS, runs, policy);
  report("v2 single-player + approved LLM-drafted bluff", false, [...BASE_STORYLETS, BLUFF], runs, policy);
  report("v2 co-op + approved LLM-drafted bluff", true, [...BASE_STORYLETS, BLUFF], runs, policy);
}

// Replay determinism: the same seed, content, and policy must reproduce the same history.
let mismatches = 0;
for (let i = 0; i < 2000; i++) {
  const a = JSON.stringify(run(5000 + i, true, [...BASE_STORYLETS, BLUFF], "wanderer"));
  const b = JSON.stringify(run(5000 + i, true, [...BASE_STORYLETS, BLUFF], "wanderer"));
  if (a !== b) mismatches++;
}
console.log(`\nreplay determinism: 2000 seeds re-run, ${mismatches} mismatches`);
