#let accent = rgb("#155EEF")
#let ink = rgb("#151922")
#let muted = rgb("#5B6472")
#let soft = rgb("#EEF4FF")
#let rule = rgb("#D7E0EA")
#let code-bg = rgb("#F6F8FA")

#set document(
  title: "QueryGraph Architecture",
  author: "QueryGraph",
  keywords: ("QueryGraph", "AI Navigator", "Croissant", "CDIF", "DID", "ODRL", "Rust"),
)

#set page(
  paper: "us-letter",
  margin: (top: 0.78in, bottom: 0.72in, x: 0.82in),
  header: context {
    if counter(page).get().first() > 1 [
      #set text(size: 8.5pt, fill: muted)
      #grid(
        columns: (1fr, auto),
        align: (left, right),
        [QueryGraph Architecture],
        [#counter(page).display("1")],
      )
      #v(3pt)
      #line(length: 100%, stroke: 0.55pt + rule)
    ]
  },
)

#set text(font: "New Computer Modern", size: 10.35pt, fill: ink, lang: "en")
#set par(justify: true, leading: 0.58em, spacing: 0.72em)
#set list(indent: 1.05em, body-indent: 0.52em, spacing: 0.28em)
#set enum(indent: 1.18em, body-indent: 0.58em, spacing: 0.28em)
#set raw(block: true)

#show link: set text(fill: accent)
#show raw.where(block: true): it => block(
  fill: code-bg,
  stroke: 0.55pt + rgb("#E2E8F0"),
  radius: 5pt,
  inset: 8pt,
  width: 100%,
)[#it]

#show heading.where(level: 1): it => {
  pagebreak(weak: true)
  block(above: 0.2in, below: 0.12in)[
    #text(size: 18pt, weight: "bold", fill: accent)[#it.body]
    #v(0.08in)
    #line(length: 100%, stroke: 0.9pt + accent)
  ]
}

#show heading.where(level: 2): it => {
  block(above: 0.12in, below: 0.04in)[
    #text(size: 13pt, weight: "bold", fill: ink)[#it.body]
  ]
}

#show heading.where(level: 3): it => {
  block(above: 0.08in, below: 0.02in)[
    #text(size: 10.8pt, weight: "bold", fill: accent)[#it.body]
  ]
}

#show heading.where(level: 4): it => {
  block(above: 0.05in, below: 0.01in)[
    #text(size: 10.2pt, weight: "bold", fill: ink)[#it.body]
  ]
}

#align(center)[
  #v(0.52in)
  #text(size: 30pt, weight: "bold", fill: accent)[QueryGraph Architecture]
  #v(0.10in)
  #text(size: 15pt, fill: muted)[Semantic Infrastructure for Agentic AI]
  #v(0.24in)
  #line(length: 58%, stroke: 1.1pt + accent)
  #v(0.26in)
  #text(size: 11pt, fill: ink)[AI Navigator semantic layer in Rust]
  #v(0.08in)
  #text(size: 9.5pt, fill: muted)[Semantic Croissant · CDIF · DID · ODRL]
  #v(0.30in)
]

#block(
  fill: soft,
  stroke: 0.65pt + rgb("#BBD2FF"),
  radius: 7pt,
  inset: 13pt,
  width: 100%,
)[
  #text(weight: "bold", fill: accent)[Report scope] \
  This document explains the meaning, architecture, and Rust implementation of QueryGraph as a four-layer semantic system for governed dataset discovery and agentic AI.
]

#v(0.32in)
#grid(
  columns: (1fr, 1fr),
  gutter: 0.18in,
  [
    #text(size: 8.5pt, fill: muted, weight: "bold")[Implementation]
    #v(2pt)
    #text(size: 9.5pt)[Rust crate with CLI, JSON-LD model builders, DID generation, ODRL policy model, and CODATA DID anchoring client.]
  ],
  [
    #text(size: 8.5pt, fill: muted, weight: "bold")[Generated from]
    #v(2pt)
    #text(size: 9.5pt)[docs/querygraph-architecture-report.md using Pandoc and Typst.]
  ],
)

#v(0.42in)
#outline(title: [Contents], depth: 2, indent: auto)

#pagebreak()

#include "querygraph-architecture-report.body.typ"
