// A start past page 1 opens where it settles: scrolled past the title and the avatar strip, the results right under
// the filter band, from the first paint. The band, the strip and the results stream as boundaries React reveals
// later (in an animation frame or a timer of its own), so this inline script (catalog/home-screen.tsx, rendered
// with them and run as it is parsed, while still hidden) watches the document and scrolls at each reveal, in the
// task that made it and so before it paints, until the first tiles are in. A reveal moves a whole region into
// place, the main region with it, so only the body sees it. The reader's own scrolling, the results hydrating (they
// set history.scrollRestoration to manual and keep the place from there, result-pages.tsx), or 10 s ends it.
// A reload, Back or Forward is the results' own restore and is left to it; a page restored from the back-forward
// cache runs no script, and React never runs a script it renders on a client-side navigation. Anything missing
// makes it do nothing.
// The line is band.tsx covered(): the band's sticky top plus its height, its strip shrunk; the top is the top bar's
// height (less the strip's shrink), which the AppShell sets on its root only once it hydrates, so this sets it
// first, as AppShell does. The results then start at that line, where the band's bottom edge meets them.

export const DEEP_START = `;(function(){var d=document,o,
I=["wheel","touchstart","keydown","pointerdown"],n=performance.getEntriesByType&&performance.getEntriesByType("navigation")[0];
if(n&&n.type!=="navigate")return;
function px(v){return parseFloat(v)||0}
function stop(){o&&o.disconnect();I.forEach(function(e){removeEventListener(e,stop,true)})}
function go(){try{if(history.scrollRestoration==="manual")return stop();
var c=d.querySelector("[data-band-content]");if(!c||c.closest("[hidden]"))return;
var b=c.previousElementSibling;if(!b)return;
var h=d.querySelector(".astryx-app-shell-header"),a=d.querySelector(".astryx-app-shell");
if(h&&a)a.style.setProperty("--_app-shell-header-height",h.getBoundingClientRect().height+"px");
var t=b.getBoundingClientRect(),e=px(getComputedStyle(b).top)+t.height,y=c.getBoundingClientRect().top;
scrollTo(0,y+scrollY-e);if(c.querySelector("[data-item]"))stop()}catch(x){stop()}}
o=new MutationObserver(go);o.observe(d.body,{childList:true,subtree:true});
I.forEach(function(e){addEventListener(e,stop,{capture:true,passive:true})});setTimeout(stop,10000);go()})();`;
