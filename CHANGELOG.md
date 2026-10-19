# Changelog

## [0.7.0](https://github.com/kevenleone/seam/compare/v0.6.0...v0.7.0) (2026-10-19)


### Features

* **cli:** list the commands when seam runs bare, and open the repository with seam . ([af0046b](https://github.com/kevenleone/seam/commit/af0046bcbd159f35ba4a09773a5870d05451e144))
* **cli:** open, review and explain diffs from the terminal with the seam command ([909c952](https://github.com/kevenleone/seam/commit/909c952ea31dd2d0a0bb98692566aaba83e2a9e0))
* **ui:** show diffs opened from the terminal in the Review tab and review them on open ([3923c3d](https://github.com/kevenleone/seam/commit/3923c3dcbcf03fb12582338c7d949d782e299ef8))


### Documentation

* describe the seam command ([00d3d44](https://github.com/kevenleone/seam/commit/00d3d44772e75d66bd428e8a29e5b53b7ff127b8))

## [0.6.0](https://github.com/kevenleone/seam/compare/v0.5.0...v0.6.0) (2026-10-18)


### Features

* **host:** give reviews, recaps and plans the project's saved overview as context ([fd427a5](https://github.com/kevenleone/seam/commit/fd427a573f662aff2d08e05cea705d556d694ee3))
* **host:** list worktrees, check out any ref detached and describe each branch's tip ([d098b27](https://github.com/kevenleone/seam/commit/d098b274607fb3b26e562d727af50c44f20af9f5))
* **host:** read a repository's facts and write an AI overview that updates only the sections that drifted ([e26b29a](https://github.com/kevenleone/seam/commit/e26b29a50dcf72bd139ad4e6d3aed064ef2273f7))
* **overview:** mark a section out of date only when its paths change enough, and small edits as minor ([c0be825](https://github.com/kevenleone/seam/commit/c0be825434a3c6ff2d679b32ef40578aebe8699b))
* **ui:** open on an Overview tab with the project's facts, architecture and freshness ([b918de5](https://github.com/kevenleone/seam/commit/b918de5a3f4b7f9719e122b03120fffcca15c6f6))
* **ui:** pick branches and workspaces from the status bar and header, and label worktree windows ([cca7f46](https://github.com/kevenleone/seam/commit/cca7f46d70e00249361ea58ea55f82b1157a2126))


### Refactoring

* **host:** share Visual's diagram tidying, repository context and JSON repair ([522aa45](https://github.com/kevenleone/seam/commit/522aa45b08dc05a62e8c41f1ce6550e14269b5a2))


### Documentation

* describe how the overview measures drift and feeds other features ([e999325](https://github.com/kevenleone/seam/commit/e999325b6539aa43eca5c5f72e976f6c62a8793e))
* describe the Overview tab ([7ec28c4](https://github.com/kevenleone/seam/commit/7ec28c40d5d9c3f84b832084b858a18fa176f48e))

## [0.5.0](https://github.com/kevenleone/seam/compare/v0.4.0...v0.5.0) (2026-10-18)


### Features

* open a branch checked out in another worktree in its own window ([278a0a2](https://github.com/kevenleone/seam/commit/278a0a2e46d35191226dfea84514e62e24d487a6))
* **ui:** show where the branch stands when there is nothing to commit ([d2428fe](https://github.com/kevenleone/seam/commit/d2428fe4d71b23d9d1d7cd76b283d717274f4a8a))


### Bug Fixes

* **ui:** bundle the wireframe sheet so sketches draw in release builds ([b5cc3e5](https://github.com/kevenleone/seam/commit/b5cc3e51a24c8bee338f03852aa09018d42aedb2))
* **ui:** make the selected tab visible in light themes ([0b8c69a](https://github.com/kevenleone/seam/commit/0b8c69a319ba15dafb777fdfdc819b627221c116))
* **ui:** size a file's diff panel to its rows, images and collapse included ([7a3215f](https://github.com/kevenleone/seam/commit/7a3215f150bbbd65c04eee5aa1dbf2d17ac705d8))


### Documentation

* warn against switching the checkout Seam Dev runs from ([4bf1cc2](https://github.com/kevenleone/seam/commit/4bf1cc2aeaf10c00be983b7da15e0cc7992b91cf))

## [0.4.0](https://github.com/kevenleone/seam/compare/v0.3.0...v0.4.0) (2026-10-17)


### Features

* **host:** keep a history of recaps and show each on its own branch ([d894341](https://github.com/kevenleone/seam/commit/d894341cb0f1e33dd1cb7004682fb223688e54b0))
* **host:** read a diff's blobs, including the unwritten working-tree side ([993dad0](https://github.com/kevenleone/seam/commit/993dad0ea3728fa6ffcc88a1b2957d8cd990cc0c))
* **host:** run the dev build as Seam Dev with its own identifier and icons ([8cbde78](https://github.com/kevenleone/seam/commit/8cbde7885d97fd6ec9b326555deecabdf095e5b5))
* **ui:** browse every recap and plan, and follow the branch you are on ([2874531](https://github.com/kevenleone/seam/commit/287453136141f722bf36e51eb3dec75baae113b4))
* **ui:** open a branch commit's changes from the recap's commit list ([4acb598](https://github.com/kevenleone/seam/commit/4acb598bf89bb4421fad7b1af356b585dfc8dc88))
* **ui:** preview changed images in the diff ([d174224](https://github.com/kevenleone/seam/commit/d174224769306f2c0462fde4df9f4759974b5593))
* **ui:** show the app's name next to its version ([2059e97](https://github.com/kevenleone/seam/commit/2059e97200a6de9c39a11f7f44b671b430af4ae7))
* **ui:** show the change and the review style while a review runs, and a richer empty state ([2fda27e](https://github.com/kevenleone/seam/commit/2fda27e7f8456d71fdfbd42b070bad6091a91f58))


### Refactoring

* **ui:** share the run status, changed files and commits between AI views ([db8aaba](https://github.com/kevenleone/seam/commit/db8aaba70b7ed3eb840b3774bc1782014091869a))


### Documentation

* describe where recaps and plans are kept ([02da809](https://github.com/kevenleone/seam/commit/02da8090ea64c7962b90bf010b29f6c7de587438))
* explain the Seam Dev config overlay ([6dd2930](https://github.com/kevenleone/seam/commit/6dd2930abc0be9339e4e3ebcefde622ef24a5382))

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
