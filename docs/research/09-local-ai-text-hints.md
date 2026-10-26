# 09: AI text hints with a small local model

**Question:** can canbec mark "this text is probably AI-written" on its own, offline, without
making the browser heavy? (Issue #25. Metadata-based image spoilers are already done, #24.)

## What exists (October 2026)

| Detector | Size | Catches AI text | Flags human text by mistake | Notes |
|---|---|---|---|---|
| Ettin-68M (ONNX int8, MIT) | 77 MB | TPR 0.77 (unseen rewrites) … 0.11 (edited chatbot answers) | fixed at 1% | runs in the browser with onnxruntime-web + tokenizers.js, first 512 tokens |
| Ettin-150M (ONNX int8, MIT) | 168 MB | similar, not better | fixed at 1% | the bigger model isn't worth it |
| Best small models in Sean Goedecke's Deckard test (RoBERTa-large INT8, a 4-bit "Gradient" model) | a few hundred MB | 52–56% | 2.5–2.7% | extension uses 400 MB–1.2 GB RAM while running |
| ModernBERT / Qwen3-0.6B fine-tunes (same test) | 150 MB–600 MB | 22–29% | 1.7–2.0% | |
| Stylometric CNN / random forest (arXiv 2511.21744) | 25 MB / 11 MB | ~95–97% on one Kaggle corpus | not reported | in-domain only, no adversarial tests |
| Commercial (Pangram, for scale) | cloud | 99.7% (claimed) | 0.004% (claimed) | not local, not an option |

Pattern across all of them: small detectors are good on text like their training data and weak on
edited or paraphrased AI text. At a 1–3% false-positive rate they still flag one human page in
40–100. On a whole browsing day that's dozens of wrong "AI" labels.

## Decision

- **Not on by default, and no auto-download.** A wrong label on a real person's writing is worse
  than no label, and 77 MB is more than canbec's whole app.
- **If we build it:** an optional download in settings (Ettin-68M, MIT), run inside a hidden
  webview with onnxruntime-web (WASM). No new Rust crates and no native ML runtime. The core
  only gets a score per page.
- **UI wording:** a quiet hint ("reads like AI, 80%") on the page's chip in the sidebar, never a
  blur or a block. The user's own slop tags and shared lists stay the strong signal.
- **Revisit** when a sub-100 MB model reaches ≥80% detection at ≤0.5% false positives on edited
  text (Beemo / MAGE paraphrased sets).

## Built (v0.3, 🧪 not yet tried in the running app)

- **Settings → AI text hints**, off by default. Turning it on downloads five files (88 MB) from
  Hugging Face (model, pinned revision) and jsDelivr (onnxruntime-web 1.22.0), each checked against
  a pinned SHA-256 (`infrastructure/src/ai_text.rs`). Turning it off deletes them. 26 s on a home
  connection.
- **Tokenizer** written out in `scripts/ai-text.js` (byte-level BPE from tokenizer.json): the same
  token ids and scores as Python `tokenizers` + onnxruntime on the test texts.
- **Runs** in a hidden webview served only from the `canbec-ai` custom protocol. One wasm thread.
  It is created on the first page to score and dropped after 10 idle minutes.
- **Speed** (M-series Mac, one thread): model ready in 0.4–1 s, a paragraph in 0.1–0.2 s, a full
  512 tokens in 1.5 s. Same in headless WebKit and node.
- **Memory**: about 500 MB while loaded. With ORT's graph optimizations on it was about 1 GB for
  the same scores and speed, so they're off.
- **What it reads**: content.js sends the longest `article`/`main` text (or the body), only with
  80+ words, at most 4,000 characters. The core scores it under its own URL for the tab.
- **Hint**: a chip "reads like AI · 91%" only at P ≥ 0.85 (`domain::ai_text::HINT_AT`, picked by
  hand: 50% already flags 5% of human texts). Never a blur or a block.

## Sources

- [AI-generated text detectors (ONNX), Hugging Face](https://huggingface.co/Deckard618/textdet)
- [Automatically detecting AI text in my browser, Sean Goedecke](https://seangoedecke.com/deckard/)
- [A Lightweight Approach to Detection of AI-Generated Texts Using Stylometric Features (arXiv 2511.21744)](https://arxiv.org/pdf/2511.21744)
- [Smaller Language Models are Better Black-box Machine-Generated Text Detectors (arXiv 2305.09859)](https://arxiv.org/pdf/2305.09859)
- [ai-text-detector-optimized (SmolLM-135M), Hugging Face](https://huggingface.co/Joshfcooper/ai-text-detector-optimized)
