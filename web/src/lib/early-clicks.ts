// Clicks made before React runs. The page is painted from the server's HTML while its code is still loading
// (seconds on a slow link), and a click on a control in that time has no handler: it is lost. React replays
// clicks only from the moment it listens on the document. This inline script (app/layout.tsx) remembers the last
// control clicked before that and clicks it again: once React listens (React then hydrates the part clicked at
// once and runs the handler), or, when that part's code was not ready yet, once React has hydrated the control.
// It relies on two names React DOM gives its expandos: `_reactListening…` on the document when its listeners
// are attached, and `__reactProps…` on each element it has hydrated.

/** Controls whose early click is replayed; links work without React and are left alone. */
export const EARLY_CONTROLS =
  'button,[role="button"],[role="checkbox"],[role="radio"],[role="switch"],[role="tab"]';

export const EARLY_CLICKS = `;(function(){var d=document,b,tried,t;
function has(o,p){return Object.keys(o).some(function(k){return k.indexOf(p)===0})}
function stop(){clearInterval(t);t=0}
function tick(){if(!b.isConnected)return stop();if(!has(d,"_reactListening"))return;
if(has(b,"__reactProps")){stop();b.click();return}
if(!tried){tried=1;b.click();if(has(b,"__reactProps"))stop()}}
d.addEventListener("click",function(e){if(has(d,"_reactListening"))return;
var c=e.target&&e.target.closest&&e.target.closest(${JSON.stringify(EARLY_CONTROLS)});if(!c)return;
b=c;tried=0;t=t||setInterval(tick,50)},true)})();`;
