---
title: Mermaid diagrams
category: Content
tags:
  - reference
---

[Mermaid](https://mermaid.js.org/) lets you describe diagrams in plain text and renders them as SVG in the browser. `aphid` recognises fenced code blocks tagged `mermaid` and emits them as `<pre class="mermaid">` elements; the bundled runtime turns them into diagrams once the page loads.

This page is a quick tour of the diagram types you'll reach for most often. For the full catalogue and every option each diagram supports, see the [Mermaid documentation](https://mermaid.js.org/intro/).

> [!NOTE]
> The runtime is loaded on demand: pages without a mermaid block don't include the script. See [[markdown]] for how the build pipeline handles this, and [[themes]] for what `base.html` needs to wire up in a custom theme.

# Flowchart

The most common diagram type — boxes and arrows. The first token after `flowchart` is the direction (`TD` top-down, `LR` left-right, `BT`, `RL`).

````markdown
```mermaid
flowchart LR
    A[Markdown file] --> B{Has frontmatter?}
    B -- yes --> C[Parse YAML]
    B -- no --> D[Skip]
    C --> E[Render to HTML]
    D --> E
```
````

```mermaid
flowchart LR
    A[Markdown file] --> B{Has frontmatter?}
    B -- yes --> C[Parse YAML]
    B -- no --> D[Skip]
    C --> E[Render to HTML]
    D --> E
```

Node shapes (`[]` rectangle, `()` rounded, `{}` diamond, `(())` circle, …) and edge styles (`-->`, `-.->`, `==>`) are documented in the [flowchart syntax reference](https://mermaid.js.org/syntax/flowchart.html).

# Sequence diagram

Useful for protocols, request/response flows, and any time-ordered interaction between actors.

````markdown
```mermaid
sequenceDiagram
    participant Browser
    participant aphid
    participant Mermaid
    Browser->>aphid: GET /wiki/mermaid/
    aphid-->>Browser: HTML + <pre class="mermaid">
    Browser->>Mermaid: parse blocks
    Mermaid-->>Browser: SVG
```
````

```mermaid
sequenceDiagram
    participant Browser
    participant aphid
    participant Mermaid
    Browser->>aphid: GET /wiki/mermaid/
    aphid-->>Browser: HTML + <pre class="mermaid">
    Browser->>Mermaid: parse blocks
    Mermaid-->>Browser: SVG
```

Arrow forms (`->>` solid, `-->>` dashed, `-)` async) and groupings (`alt`, `loop`, `par`, `note over`) are covered in the [sequence diagram reference](https://mermaid.js.org/syntax/sequenceDiagram.html).

# Class diagram

For data models and type relationships.

````markdown
```mermaid
classDiagram
    class Page~F~ {
        +String slug
        +String body
        +F frontmatter
        +url_path() String
    }
    class BlogFrontmatter {
        +String title
        +Date created
        +Vec~Tag~ tags
    }
    class WikiFrontmatter {
        +String title
        +Option~String~ category
    }
    Page <|-- BlogFrontmatter : F =
    Page <|-- WikiFrontmatter : F =
```
````

```mermaid
classDiagram
    class Page~F~ {
        +String slug
        +String body
        +F frontmatter
        +url_path() String
    }
    class BlogFrontmatter {
        +String title
        +Date created
        +Vec~Tag~ tags
    }
    class WikiFrontmatter {
        +String title
        +Option~String~ category
    }
    Page <|-- BlogFrontmatter : F =
    Page <|-- WikiFrontmatter : F =
```

Visibility markers (`+`, `-`, `#`), generics with `~T~`, and relationship arrows (`<|--` inheritance, `*--` composition, `o--` aggregation) are listed in the [class diagram reference](https://mermaid.js.org/syntax/classDiagram.html).

# State diagram

For finite state machines and lifecycle flows.

````markdown
```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Loading : start build
    Loading --> Indexed : pass 1 done
    Indexed --> Rendering : pass 2 starts
    Rendering --> Written : write dist/
    Written --> [*]
    Rendering --> Failed : error
    Failed --> [*]
```
````

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Loading : start build
    Loading --> Indexed : pass 1 done
    Indexed --> Rendering : pass 2 starts
    Rendering --> Written : write dist/
    Written --> [*]
    Rendering --> Failed : error
    Failed --> [*]
```

Composite states, choice points, and forks/joins are described in the [state diagram reference](https://mermaid.js.org/syntax/stateDiagram.html).

# Entity-relationship diagram

For database schemas and domain models.

````markdown
```mermaid
erDiagram
    POST ||--o{ TAG : "tagged with"
    POST }o--|| AUTHOR : "written by"
    POST {
        string slug PK
        string title
        date created
    }
    TAG {
        string slug PK
        string name
    }
    AUTHOR {
        string name PK
        string email
    }
```
````

```mermaid
erDiagram
    POST ||--o{ TAG : "tagged with"
    POST }o--|| AUTHOR : "written by"
    POST {
        string slug PK
        string title
        date created
    }
    TAG {
        string slug PK
        string name
    }
    AUTHOR {
        string name PK
        string email
    }
```

Cardinality markers (`||` exactly one, `o{` zero or more, `|{` one or more) and attribute keys (`PK`, `FK`, `UK`) are detailed in the [ER diagram reference](https://mermaid.js.org/syntax/entityRelationshipDiagram.html).

# Gantt chart

For project timelines and roadmaps.

````markdown
```mermaid
gantt
    title aphid roadmap
    dateFormat YYYY-MM-DD
    section v0.1
    Core pipeline       :done, 2026-01-01, 2026-03-01
    Themes & templates  :done, 2026-03-01, 2026-04-15
    section v0.1.1
    Mermaid + alerts    :done, 2026-04-15, 2026-04-30
    section v0.2
    Drafts & RSS        :active, 2026-05-01, 30d
```
````

```mermaid
gantt
    title aphid roadmap
    dateFormat YYYY-MM-DD
    section v0.1
    Core pipeline       :done, 2026-01-01, 2026-03-01
    Themes & templates  :done, 2026-03-01, 2026-04-15
    section v0.1.1
    Mermaid + alerts    :done, 2026-04-15, 2026-04-30
    section v0.2
    Drafts & RSS        :active, 2026-05-01, 30d
```

Task states (`done`, `active`, `crit`), dependencies via `after taskId`, and milestones with `:milestone,` are in the [Gantt reference](https://mermaid.js.org/syntax/gantt.html).

# Mindmap

Useful for hierarchical brainstorms.

````markdown
```mermaid
mindmap
  root((aphid))
    Content
      Blog
      Wiki
      Pages
    Pipeline
      Pass 1: index
      Pass 2: render
    Output
      build → dist/
      serve → live reload
```
````

```mermaid
mindmap
  root((aphid))
    Content
      Blog
      Wiki
      Pages
    Pipeline
      Pass 1: index
      Pass 2: render
    Output
      build → dist/
      serve → live reload
```

Node shape syntax matches flowcharts (`(())`, `[]`, `{{}}`, …). See the [mindmap reference](https://mermaid.js.org/syntax/mindmap.html).

# Theming

Diagram colours can be tuned per site by overriding `themeVariables` when calling `mermaid.initialize` in your theme's `base.html` — see [[themes]] for a worked example. The full set of theme variables is documented in the [Mermaid theming guide](https://mermaid.js.org/config/theming.html).

# Other diagram types

Mermaid also supports pie charts, quadrant charts, requirement diagrams, user journeys, gitgraph, C4 diagrams, timelines, sankey charts, XY charts, and block diagrams. They all use the same fenced-block mechanism — just change the first line. The [Mermaid syntax index](https://mermaid.js.org/syntax/) lists them all.

See also: [[markdown]], [[themes]].
