// Clicks made before the page has hydrated. The page is painted from the server's HTML while its code is still
// loading (seconds on a slow link). A click on a control then is lost: before React listens there is no handler,
// and afterwards React tries to hydrate the clicked part at once and, when that part's code has not arrived yet,
// stops the event and drops it. This inline script (app/layout.tsx) watches clicks on controls from the document's
// capture phase, ahead of React's listener: a click React did not handle (it was not listening yet, or it stopped
// the event in that phase) is kept, and the control is clicked again once React listens and has hydrated it, until
// one click gets through (attempts at least 300 ms apart, for 10 s at most).
// It relies on two names React DOM gives its expandos: `_reactListening…` on the document when its listeners are
// attached, and `__reactProps…` on each element it has hydrated.

/** Controls whose early click is replayed; links work without React and are left alone. */
const EARLY_CONTROLS =
  'button,[role="button"],[role="checkbox"],[role="radio"],[role="switch"],[role="tab"]';

export const EARLY_CLICKS = `;(function(){var d=document,c,t,last=0,until=0;
function has(o,p){return Object.keys(o).some(function(k){return k.indexOf(p)===0})}
function stop(){clearInterval(t);t=0;c=null}
function tick(){var now=Date.now();if(!c||!c.isConnected||now>until)return stop();
if(!has(d,"_reactListening")||now-last<300)return;if(last&&!has(c,"__reactProps"))return;last=now;c.click()}
d.addEventListener("click",function(e){var x=e.target&&e.target.closest&&e.target.closest(${JSON.stringify(EARLY_CONTROLS)});if(!x)return;
var listening=has(d,"_reactListening"),blocked=0,sp=e.stopPropagation;
if(listening&&has(x,"__reactProps")&&x!==c)return;
e.stopPropagation=function(){if(e.eventPhase===1&&e.currentTarget===d)blocked=1;return sp.call(e)};
setTimeout(function(){if(listening&&!blocked){if(x===c)stop();return}
if(x!==c){c=x;last=0;until=Date.now()+10000}t=t||setInterval(tick,50)},0)},true)})();`;
