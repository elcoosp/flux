// Deliberately invalid Flux source: the `Text(` call is never closed. Used by
// `stdlib/parse-check.sh --self-test` to prove the harness actually reports
// parse failures instead of silently passing everything.

compo Broken
  Text("unterminated
