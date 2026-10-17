# Changelog

## [0.3.0](https://github.com/kevenleone/seam/compare/v0.2.0...v0.3.0) (2026-10-17)


### Features

* **host:** draw visual recaps and plans with the configured AI ([bb3d9c2](https://github.com/kevenleone/seam/commit/bb3d9c2e67a834f5e5eb75ef1efb6896e1f38ff5))
* **host:** install and remove the seam command in PATH ([a5be3e1](https://github.com/kevenleone/seam/commit/a5be3e194a6b3101738c20c998d14fb0772ce78d))
* **host:** notify and play a sound when an AI run finishes ([cf88d32](https://github.com/kevenleone/seam/commit/cf88d32963309005dbd91c11cb69d8fdf60e87df))
* **ui:** add the Visual tab with recaps, plans and exports ([3ac4b82](https://github.com/kevenleone/seam/commit/3ac4b82e48154c0b8d2e947fb8899eca5b411468))
* **ui:** configure notifications and sounds in Settings › General ([328ac16](https://github.com/kevenleone/seam/commit/328ac16ddc56cdca67945523d004112d54e7873c))
* **ui:** highlight code in diffs, with a setting to turn it off ([77c83f2](https://github.com/kevenleone/seam/commit/77c83f23375ce6e9ff66de13296518b418ed5753))
* **ui:** install the seam command from Settings › General ([2d4d05d](https://github.com/kevenleone/seam/commit/2d4d05d6395dd6c7a7396b858cc2bd3255a48d61))
* **ui:** show the change while a visual draws, and fit, width and file diffs ([afb4829](https://github.com/kevenleone/seam/commit/afb482926883052cb606bd98e95f46ac110e0a23))


### Bug Fixes

* **site:** stop screenshots being dragged or selected ([01f94fd](https://github.com/kevenleone/seam/commit/01f94fd83a6a306de10bab9eb7f107f3d3016b68))
* **ui:** let an exported visual scroll and be selected ([670eb84](https://github.com/kevenleone/seam/commit/670eb84c7ee8551e87262176243e26bfbc54c178))
* **ui:** wrap long lines in split diffs so both sides stay in view ([35c9277](https://github.com/kevenleone/seam/commit/35c92771de974f7479c36648fafd4c8b9bb80f3f))


### Refactoring

* **engine:** share JSON extraction and diff clipping across AI features ([fa35dcc](https://github.com/kevenleone/seam/commit/fa35dcc7e7011943ccece0d25947b642194817de))
* **ui:** share the provider picker and running panel ([ce7ed8e](https://github.com/kevenleone/seam/commit/ce7ed8e9f2533ac08e86e6e7d0dab17ed8099414))


### Documentation

* describe the seam command ([5df2e16](https://github.com/kevenleone/seam/commit/5df2e1690ba949aed02af7d856cf17d9094a6226))
* describe visual recaps and plans ([adb51aa](https://github.com/kevenleone/seam/commit/adb51aa515407ca5fbe4fb7b37a3bdb9965dacbe))
* retake screenshots with syntax highlighting ([b5fd6e5](https://github.com/kevenleone/seam/commit/b5fd6e5aa3a0c3d7f954641ae9d72e223cedfaa2))

## [0.2.0](https://github.com/kevenleone/seam/compare/v0.1.0...v0.2.0) (2026-10-17)


### Features

* **ai:** talk to coding agents and model APIs natively ([0bdb2dc](https://github.com/kevenleone/seam/commit/0bdb2dc4409460ccc27f1f5cf56dbeebc28316c9))
* **engine:** parse git status, diffs, history and refs ([e9c7f3d](https://github.com/kevenleone/seam/commit/e9c7f3d3fd4dd3e78c6e1b12a3d65b555714e226))
* **engine:** stage single lines, rewrite history and undo every action ([265717e](https://github.com/kevenleone/seam/commit/265717e5a45beab1fcbf37cae1534aa6670a6184))
* **host:** add the menu bar icon, saved preferences and a settings window ([9eb5449](https://github.com/kevenleone/seam/commit/9eb5449746f96af67d06b985705f71571f26fc80))
* **host:** open repository windows with menus and file watching ([3e8167c](https://github.com/kevenleone/seam/commit/3e8167c991242507cad2de90a6eab2bede65f571))
* **site:** add the website and deploy it to GitHub Pages ([8b6b79c](https://github.com/kevenleone/seam/commit/8b6b79c7abb0b53c246a685b88216c29d2848c9c))
* **ui:** add the diff view, command palette and dialogs ([01813c2](https://github.com/kevenleone/seam/commit/01813c2ee19c4632ba3359bf4f7e92565ed4fb54))
* **ui:** add the Review tab with comments on the diff ([34c8bf6](https://github.com/kevenleone/seam/commit/34c8bf6d9f449d2b4e605bc636389edacb8f62f7))
* **ui:** add the welcome, changes and history views ([98b7fdc](https://github.com/kevenleone/seam/commit/98b7fdc838fc325ded76d207bfdd7a80db3aa718))
* **ui:** add theme tokens, the typed API and the repository store ([1ffe8aa](https://github.com/kevenleone/seam/commit/1ffe8aa948afdff90a834a2376a09b0e64209541))
* **ui:** add themes, accents and the settings sheet ([f2a79a5](https://github.com/kevenleone/seam/commit/f2a79a5cef891c23b8d6395d1bbb3edbec14e0c6))
* **ui:** expand the diff and tick commits with ⌘-click and ⇧-click ([ea17612](https://github.com/kevenleone/seam/commit/ea176126ce137babc4010b32bd7a57cfa14f1db4))


### Bug Fixes

* **review:** keep finding titles whole and the provider picker compact ([5a3801b](https://github.com/kevenleone/seam/commit/5a3801b1d95d7c8c3aee75596dc2c3044dcc532c))
* **tray:** stop ⌘, opening settings twice ([65470de](https://github.com/kevenleone/seam/commit/65470de098bb79037439411be48c16a823c7484a))
* **ui:** let Tailwind utilities style buttons and inputs ([8a8972d](https://github.com/kevenleone/seam/commit/8a8972d262e302bafb2390ae82e7016bc5a6ac5c))


### Documentation

* add README ([986c9fc](https://github.com/kevenleone/seam/commit/986c9fc0727c61b4892da7081441b6a412d5d619))
* add screenshots ([1118df3](https://github.com/kevenleone/seam/commit/1118df344dfe89b1045800f7ef4924c045ce7ba9))
* describe AI providers, themes and settings ([ab9293a](https://github.com/kevenleone/seam/commit/ab9293a57594f0f61223219e67e438f7047c0067))
* take every screenshot in Seam Dark ([304e193](https://github.com/kevenleone/seam/commit/304e193d8f5e25f137c6615e58492118f2493904))


### Build System

* scaffold the Tauri, Vite and React app ([deb3a8c](https://github.com/kevenleone/seam/commit/deb3a8cc1637218813619d90e884001289855e8d))
