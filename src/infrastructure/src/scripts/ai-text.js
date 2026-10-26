// canbec AI text hints (#25): Ettin-68M (MIT) behind onnxruntime-web, offline, in a hidden webview.
// Only runs after the user downloads the model in settings. Also loads in node for the parity test.
// The tokenizer is tokenizer.json's byte-level BPE, written out here instead of a 1 MB library.
// ponytail: added tokens in page text (e.g. "<|endoftext|>") are split like normal text, not matched.
const canbecAiText = (() => {
  const PIECES = /'s|'t|'re|'ve|'m|'ll|'d| ?\p{L}+| ?\p{N}+| ?[^\s\p{L}\p{N}]+|\s+(?!\S)|\s+/gu;
  const MAX_TOKENS = 512;

  // GPT-2's byte → printable character table.
  const byteChar = [];
  for (let b = 0, n = 0; b < 256; b++) {
    const printable = (b >= 33 && b <= 126) || (b >= 161 && b <= 172) || b >= 174;
    byteChar[b] = String.fromCharCode(printable ? b : 256 + n++);
  }

  function tokenizer(json) {
    const vocab = json.model.vocab;
    const rank = new Map(json.model.merges.map((m, i) => [Array.isArray(m) ? m.join(" ") : m, i]));
    const special = (t) => json.added_tokens.find((a) => a.content === t).id;
    const [cls, sep] = [special("[CLS]"), special("[SEP]")];
    const cache = new Map();
    const utf8 = new TextEncoder();

    function bpe(piece) {
      if (cache.has(piece)) return cache.get(piece);
      let word = [...utf8.encode(piece)].map((b) => byteChar[b]);
      while (word.length > 1) {
        let best = -1, bestRank = Infinity;
        for (let i = 0; i < word.length - 1; i++) {
          const r = rank.get(word[i] + " " + word[i + 1]);
          if (r !== undefined && r < bestRank) [best, bestRank] = [i, r];
        }
        if (best < 0) break;
        const [a, b] = [word[best], word[best + 1]];
        const merged = [];
        for (let i = 0; i < word.length; i++) {
          if (i < word.length - 1 && word[i] === a && word[i + 1] === b) merged.push(a + b), i++;
          else merged.push(word[i]);
        }
        word = merged;
      }
      const ids = word.map((s) => vocab[s]).filter((id) => id !== undefined);
      if (cache.size < 50000) cache.set(piece, ids);
      return ids;
    }

    // The normalizer from tokenizer.json: NFKC, invisible characters out, whitespace collapsed.
    const normalize = (text) =>
      text.normalize("NFKC")
        .replace(/[\u00ad\u180e\u200b-\u200f\u202a-\u202e\u2060-\u2064\u2066-\u2069\ufeff]/g, "")
        .replace(/[\t\n\u000b\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+/g, " ")
        .trim()
        .normalize("NFC");

    return (text) => {
      const ids = [];
      for (const [piece] of normalize(text).matchAll(PIECES)) {
        ids.push(...bpe(piece));
        if (ids.length >= MAX_TOKENS - 2) break;
      }
      return [cls, ...ids.slice(0, MAX_TOKENS - 2), sep];
    };
  }

  // P(AI-generated) for a text: softmax over the model's [human, AI] logits.
  async function detector(ort, model, tokenizerJson) {
    // Graph optimizations off: same scores and speed, about half the memory (~500 MB, not ~1 GB).
    const session = await ort.InferenceSession.create(model, { graphOptimizationLevel: "disabled" });
    const encode = tokenizer(tokenizerJson);
    return async (text) => {
      const ids = encode(text);
      const tensor = (fill) => new ort.Tensor("int64", BigInt64Array.from(ids, fill), [1, ids.length]);
      const out = await session.run({ input_ids: tensor((id) => BigInt(id)), attention_mask: tensor(() => 1n) });
      const [human, ai] = out[session.outputNames[0]].data;
      return 1 / (1 + Math.exp(human - ai));
    };
  }

  return { tokenizer, detector };
})();
if (typeof module !== "undefined") module.exports = canbecAiText;
