// Ambient module declarations for Starlight's virtual component modules.
//
// Starlight exposes its internal UI components through `virtual:starlight/components/*`
// deep imports (resolved by Vite at build time). `astro check` (the TypeScript
// language server) cannot resolve these virtual module specifiers, so we declare
// them here as Astro components to keep the type-check green without affecting
// runtime behavior.

declare module 'virtual:starlight/components/SiteTitle' {
  const SiteTitle: import('astro').AstroComponentFactory;
  export default SiteTitle;
}
declare module 'virtual:starlight/components/Search' {
  const Search: import('astro').AstroComponentFactory;
  export default Search;
}
declare module 'virtual:starlight/components/SocialIcons' {
  const SocialIcons: import('astro').AstroComponentFactory;
  export default SocialIcons;
}
declare module 'virtual:starlight/components/ThemeSelect' {
  const ThemeSelect: import('astro').AstroComponentFactory;
  export default ThemeSelect;
}
declare module 'virtual:starlight/components/LanguageSelect' {
  const LanguageSelect: import('astro').AstroComponentFactory;
  export default LanguageSelect;
}
