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
