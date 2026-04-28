---
title: Creating a theme with Claude Design
slug: theme-creation-claude-design
author: LHelge
created: 2026-04-26
image: /static/blog/claude.svg
description: How I used Claude Design to create the default aphid theme and how to build your own theme in a similar way.
tags:
  - design
  - themes
---

The default aphid theme is bare-bones — a white background, the content, and no styling whatsoever. Its main job is end-to-end testing. For the project's own blog and docs I wanted something that actually looked good, and building it with aphid itself is the best kind of [dogfooding](https://en.wikipedia.org/wiki/Eating_your_own_dog_food).

What I had in mind was a layout that worked for long-form wiki pages and short blog posts alike, with sensible typography, a sidebar for navigation, and syntax-highlighted code blocks that didn't look like an afterthought. Rather than start from a CSS framework or copy an existing Hugo theme, I used [Claude Design](https://claude.ai) as a design collaborator.

**What you see here is the result.**

Claude Design has plenty of features for steering the direction of a design, but I gave it fairly free rein. The one piece of guidance I did give it was that I like the [Catppuccin](https://catppuccin.com/) Mocha palette. Beyond that I let it lead, and iterated on what came back.

# Try for yourself

Claude Design is included in all paid plans, though usage on the basic $10 tier is fairly limited. Even there you should be able to push through one or two designs before hitting the cap.

The intended workflow is to set up a design system and feed in a lot of design-related context before you start prompting, but that's optional. I've had good results with very little steering — what you do need to give Claude is a clear picture of the application's structure and what each template is for. There are more pointers on the [[ai-assisted-design]] wiki page.
