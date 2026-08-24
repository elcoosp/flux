// Deliberately invalid Flux source: the `Text(` call is never closed and the
// component body has no `}`. Used by `stdlib/parse-check.sh --self-test` to
// prove the harness actually reports parse failures instead of silently
// passing everything.
component Broken {
  Text("unterminated
