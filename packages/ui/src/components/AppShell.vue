<template>
  <div class="rss-ui shell">
    <header><slot name="header" /></header>
    <div class="body">
      <aside>
        <div class="navigation-scroll"><slot name="navigation" /></div>
        <div class="navigation-footer"><slot name="navigation-footer" /></div>
      </aside>
      <main><slot /></main>
    </div>
    <footer><slot name="status" /></footer>
  </div>
</template>
<style scoped>
.shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--color-bg);
}
header,
footer {
  flex: none;
  padding: 16px 24px;
  background: var(--color-surface);
}
header {
  border-bottom: 1px solid var(--color-border);
}
footer {
  border-top: 1px solid var(--color-border);
}
.body {
  display: flex;
  flex: 1;
  min-height: 0;
}
aside {
  flex: 0 0 200px;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 16px;
  border-right: 1px solid var(--color-border);
}
.navigation-scroll {
  overflow: auto;
  min-height: 0;
  flex: 1;
}
.navigation-footer {
  flex: none;
  padding-top: 12px;
}
main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  padding: 24px;
}
@media (max-width: 640px) {
  .body {
    flex-direction: column;
  }
  aside {
    flex: none;
    max-height: 140px;
    border-right: 0;
    border-bottom: 1px solid var(--color-border);
  }
  .navigation-scroll {
    overflow: auto;
    min-height: 0;
    flex: 1;
  }
  .navigation-footer {
    flex: none;
    padding-top: 12px;
  }
  main {
    padding: 16px;
  }
}
</style>
