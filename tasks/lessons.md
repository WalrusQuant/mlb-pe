# Lessons

## Overflow clips InfoTip

`position: absolute` tooltips inside a parent with `overflow-x: auto` get clipped on **both** axes. CSS treats the other axis as `auto` when one is not `visible`. Fix at the component: portal the tip to `document.body` and use `position: fixed` from the trigger’s bounding rect. Do not try to “just use overflow-y: visible.”
