# Erzählstudio

<img src="src/web/assets/erzaehlstudio-logo.svg" alt="Erzählstudio logo" width="72" height="72" />

**A place to create worlds where choices matter.**

Erzählstudio aspires to make interactive storytelling feel as natural as imagining a world: bring its people to life, connect their stories, and discover what happens when a player chooses differently.

![Erzählstudio web app — a visual workspace for branching stories](docs/images/ui-rework-after-storylet.webp)

## Our ambition

Give writers and game makers room to think beyond a single path. From an intimate mystery to a sprawling role-playing world, the goal is stories shaped by characters, knowledge, place, and consequence—not just the next line of dialogue.

Keep the creative process close: imagine, write, arrange, play, reconsider. A calm workspace should help creators see how their stories fit together without taking attention away from the stories themselves.

![Erzählstudio web app — a flexible workspace with stories and world details side by side](docs/images/ui-rework-split-view.webp)

## Build your world with entities and traits

An **entity** is something in your story world: a person, a place, an item, or a fact. Give it an identity, then describe what matters to your story.

A **trait** is a reusable set of properties. Instead of defining the same details for every character or location, define them once in a trait and give each entity its own values.

For example, a **Lighthouse Keeper** trait could describe years of service and whether the keeper has a key. Two keepers can share that trait while having different histories and access.

Combine traits rather than forcing everything into one category. A character can also be a merchant or an inventory owner; each trait adds another part of what that entity means in your world.

An **entity template** is a reusable starting point for similar entities. Put shared traits and starting values on a template, then create individual entities from it. A *Keeper* template might give every keeper a key by default, while *Mara* and *Jo* have their own names and years of service.

## Try it: create a lighthouse keeper

1. In the project tree, open the menu on **Traits** and choose **New Trait**. Name it *Lighthouse Keeper* with the key `lighthouse_keeper`. Under **Typed Properties**, add *Years of service* (`years_of_service`, integer) and *Has a key* (`has_key`, boolean). Set starting values and click **Save**.
2. Open the menu on **Entity Templates** and choose **New Entity Template**. Name it *Keeper*. In its **Traits** tab, select *Lighthouse Keeper* from **Choose trait…**, click **Add**, and **Save**.
3. Open the menu on **Entities** and choose **New Entity**. Name it *Mara* and choose *Keeper* as its **Archetype template**.
4. In Mara’s **Traits** tab, set her years of service and whether she has a key. Click **Save**. These are Mara’s values; another keeper can have different ones.
5. Create *Jo* from the same template and give Jo different values. You now have two distinct people built from the same reusable design.

You can also add traits directly to an entity without using an entity template. Use templates for shared starting points, traits for reusable details, and entities for the individual things in your world.

**World** is the studio’s built-in entity, not a character template. Keep your people, places, and items separate from it.

## Let stories respond to the world

A **storylet** is a small playable scene: a conversation, an encounter, a discovery, or a turning point. Rather than putting every scene on one fixed path, you describe when each scene makes sense.

Think of a locked lighthouse:

1. **Set the situation.** The scene can require the player to be at the lighthouse and meet conditions you choose.
2. **Write the moment.** Add dialogue and connect it to choices in the story graph.
3. **Give choices consequences.** A choice might change a property, reveal information, or move the player somewhere else.
4. **Connect what comes next.** Finish the scene, jump to another storylet, or mark an ending for the run.
5. **Play and reconsider.** Save and approve the storylet so it can take part in a run. Use **Play** to try your story, rewind to a choice, and explore a different outcome.

During play, the studio checks the current world and participants to determine which storylets can happen. A change made in one scene can make another scene available—or prevent it from happening. Your scenes become parts of a responsive world, not just branches on a single tree.

## Extend your creative tools with plugins

A **plugin** adds tools to the studio without making every project use them. It can add a new tab on an entity, a more useful way to edit a property, or new kinds of steps in a story graph.

The built-in plugins show what this means in practice:

- **Vector Canvas** gives an entity a visual canvas for maps, notes, and references to people, places, and stories.
- **Inventory** lets you give entities inventories, arrange items, and use story choices to add, remove, or check those items.
- **Tree Graph** lets you arrange connected entities visually—for example, as a skill tree or relationship map.

Open **Plugins** to turn installed tools on or off. Then add the relevant trait to an entity: **Canvas** for a canvas, **InventoryOwner** for an inventory, or **TreeGraph** for a tree. Its extra editor appears as a tab on that entity.

For example, give Mara the **InventoryOwner** trait and give a lighthouse-key entity the **Item** trait. A story choice can check whether she carries that key before opening the door. The plugin supplies the tool; you decide what it means for the story.

Custom plugins can extend these same authoring tools, but creating one currently requires development work; there is no plugin marketplace or file-install button. Use only plugins you trust.
