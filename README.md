# vercel-anti-bot
Reverse engineering and analysis of Vercel's bot protection used on https://sdk.vercel.ai
(and potentially more of their platforms).

## Usage
The `generate_token` function in `src/lib.rs` takes in the data from the `/openai.jpeg` response,
which returns a valid token for usage in the `custom-encoding` header on a protected request.

While this repository does not provide request automation, you can generate a token and replay
a request from the browser with the generated token. Keep in mind the data returned from `/openai.jpeg`
seems to be *very* short-lived.

Disclaimer: this repository is intended for criticism only.

## Background
I first became aware of this after seeing [this tweet](https://twitter.com/jaredpalmer/status/1675192755763412992?s=20)
from Vercel's VP claiming they have reduced costs by 100x since implementing this solution (as well as rate limiting).
The tweet claims their solution is "quite promising" and encouraged anyone interested to contact him.

This sounds convincing at first, but when taking a look at how their bot protection works, unfortunately it's *very easy*.
This is extremely disappointing especially if you read [this reply](https://twitter.com/jaredpalmer/status/1675196288831311876?s=20)
claiming Vercel's CTO who previously ran Google Search built this bot protection system.
For clarification, a system like this can be built easily by anyone in the cybersecurity space, and a lot of people -
including myself - can easily do better.

The analysis below explains how the system works and how this repository circumvents it.

## Analysis
If you navigate to https://sdk.vercel.ai, open DevTools, navigate to the Sources tab and then
use the Search feature at the bottom using this filter:

`file:* function useAntibotToken()`

You should come across a function in a JavaScript file that looks like this:
```js
function useAntibotToken() {
    let {data, mutate, isValidating} = (0,
        swr__WEBPACK_IMPORTED_MODULE_0__.ZP)("antibot-token", async()=>{
            let response = await fetch("/openai.jpeg")
                , data = JSON.parse(atob(await response.text()))
                , ret = eval("(".concat(data.c, ")(data.a)"));
            return btoa(JSON.stringify({
                r: ret,
                t: data.t
            }))
        }
        , {
            fallbackData: "",
            refreshInterval: 6e4,
            dedupingInterval: 2e3
        });
    return [data, mutate]
}
```

From this code, we can see that:
1) The browser makes a request to https://sdk.vercel.ai/openai.jpeg.
2) The response is base64 decoded and parsed as JSON.
3) The following code is evaluated using the `eval` function: `(c)(data.a)`, where `c` is the `c` property of the JSON object.
4) The function returns a base64 encoded JSON object, with `r` being the evaluated value and `t` being the `t` property from the JSON object.

The response from the `/openai.jpeg` request is a large string. For this example, we'll be using this one:
```
eyJ0IjoiZXlKaGJHY2lPaUprYVhJaUxDSmxibU1pT2lKQk1qVTJSME5OSW4wLi45UnRnbGU3VmtaVW80N1VwLjZCZkFkYkRnMERuVFJfcDJhb0JhMzhDMktYZHp0bEdKaHppem5kdzBsRGJZUWNLRjRwMjVRckhqYV9ZWG5IY3V2UkhDNURMZFJyTm9iYU5DeThVMXZ2OVVsWnlXdHFsU3VSUEdhdkpsVzNIZnp5VzlRN2JwQUJTMmtQQ1dWWTAuWFd6b1I2Ym5HTmVjaEJESlZZMXB6dyIsImMiOiJmdW5jdGlvbihhKXsoZnVuY3Rpb24oZSxzKXtmb3IodmFyIHQ9eCxuPWUoKTtbXTspdHJ5e3ZhciBpPXBhcnNlSW50KHQoMzA1KSkvMStwYXJzZUludCh0KDMwNykpLzIqKC1wYXJzZUludCh0KDMxMCkpLzMpK3BhcnNlSW50KHQoMzAzKSkvNCstcGFyc2VJbnQodCgyOTkpKS81K3BhcnNlSW50KHQoMzAyKSkvNistcGFyc2VJbnQodCgzMDApKS83KigtcGFyc2VJbnQodCgzMDkpKS84KStwYXJzZUludCh0KDMwMSkpLzkqKC1wYXJzZUludCh0KDMwNCkpLzEwKTtpZihpPT09cylicmVhaztuLnB1c2gobi5zaGlmdCgpKX1jYXRjaHtuLnB1c2gobi5zaGlmdCgpKX19KShyLDEyMjA5MSoxKzY2NTQ3NCstMjkzMzM3KTtmdW5jdGlvbiB4KGUscyl7dmFyIHQ9cigpO3JldHVybiB4PWZ1bmN0aW9uKG4saSl7bj1uLSgtNTM1KzQzMyozKy00NjcpO3ZhciBjPXRbbl07cmV0dXJuIGN9LHgoZSxzKX1mdW5jdGlvbiByKCl7dmFyIGU9W1wiNDYxMzRpbGdWU09cIixcIjI2NjA3NDRqaEdtb1BcIixcIjMzODYwMHhlR25iSFwiLFwiOTY2NjY5aHZRSXBPXCIsXCJMTjEwXCIsXCI2MjA3MnBEZnVOc1wiLFwibG9nMlwiLFwiOGdQZmRJaVwiLFwiNjlmRnNXZmFcIixcImtleXNcIixcIm1hcmtlclwiLFwicHJvY2Vzc1wiLFwiMzQzNDAwNW1EWmN1elwiLFwiNTM0MjQ5MU1HYk93WFwiLFwiMTM1dE9CZGR2XCJdO3JldHVybiByPWZ1bmN0aW9uKCl7cmV0dXJuIGV9LHIoKX1yZXR1cm4gZnVuY3Rpb24oKXt2YXIgZT14O3JldHVyblthL01hdGhbZSgzMDgpXShhKk1hdGhbZSgzMDYpXSksT2JqZWN0W2UoMzExKV0oZ2xvYmFsVGhpc1tlKDI5OCldfHx7fSksZ2xvYmFsVGhpc1tlKDI5NyldXX0oKX0iLCJhIjowLjUyNTY4ODU3Mjk2MDM1NDR9
```

We can use this simple Python script and run it in the terminal to see what the JSON object is.
```python
import base64
import json
raw_data = "" # the data you want to decode
decoded_data = base64.b64decode(raw_data)
data = json.loads(decoded_data)
with open("data.json", "w") as f: # change file name to anything you like
    json.dump(data, f)
```
Now that we've decoded the data, we can see the JSON object:
```json
{
   "t": "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..9Rtgle7VkZUo47Up.6BfAdbDg0DnTR_p2aoBa38C2KXdztlGJhzizndw0lDbYQcKF4p25QrHja_YXnHcuvRHC5DLdRrNobaNCy8U1vv9UlZyWtqlSuRPGavJlW3HfzyW9Q7bpABS2kPCWVY0.XWzoR6bnGNechBDJVY1pzw",
   "c": "function(a){(function(e,s){for(var t=x,n=e();[];)try{var i=parseInt(t(305))/1+parseInt(t(307))/2*(-parseInt(t(310))/3)+parseInt(t(303))/4+-parseInt(t(299))/5+parseInt(t(302))/6+-parseInt(t(300))/7*(-parseInt(t(309))/8)+parseInt(t(301))/9*(-parseInt(t(304))/10);if(i===s)break;n.push(n.shift())}catch{n.push(n.shift())}})(r,122091*1+665474+-293337);function x(e,s){var t=r();return x=function(n,i){n=n-(-535+433*3+-467);var c=t[n];return c},x(e,s)}function r(){var e=[\"46134ilgVSO\",\"2660744jhGmoP\",\"338600xeGnbH\",\"966669hvQIpO\",\"LN10\",\"62072pDfuNs\",\"log2\",\"8gPfdIi\",\"69fFsWfa\",\"keys\",\"marker\",\"process\",\"3434005mDZcuz\",\"5342491MGbOwX\",\"135tOBddv\"];return r=function(){return e},r()}return function(){var e=x;return[a/Math[e(308)](a*Math[e(306)]),Object[e(311)](globalThis[e(298)]||{}),globalThis[e(297)]]}()}",
   "a": 0.5256885729603544
}
```

We can now see that the `c` property is a JavaScript function that has one parameter, `a`, which is the `a` property
of the JSON object as we mentioned previously from looking at the `eval` code. The `t` property doesn't appear to be used
in the code (at least from what we know so far) and is only used as a field in the encoded JSON object that is returned.

If you take the `c` property and paste it into https://beautifier.io/, the code is now much easier to read:
```js
function(a) {
    function x(e, s) {
        var t = r();
        return x = function(n, i) {
            n = n - (71 * -137 + 5097 + 4754);
            var c = t[n];
            return c
        }, x(e, s)
    }
    return function(e, s) {
            for (var t = x, n = e();
                [];) try {
                var i = -parseInt(t(135)) / 1 + parseInt(t(126)) / 2 + -parseInt(t(124)) / 3 * (parseInt(t(128)) / 4) + -parseInt(t(130)) / 5 + parseInt(t(133)) / 6 * (parseInt(t(131)) / 7) + parseInt(t(132)) / 8 + parseInt(t(125)) / 9;
                if (i === s) break;
                n.push(n.shift())
            } catch {
                n.push(n.shift())
            }
        }(r, -170842 + -1 * 92122 + 375877),
        function() {
            var e = x;
            return [a * Math[e(127)](a * Math.E), Object[e(134)](globalThis[e(129)] || {}), globalThis[e(136)]]
        }();

    function r() {
        var e = ["7WUOLfS", "406424fiusCg", "293790OLgwin", "keys", "176487LGrtxs", "data", "69177FwHYUB", "1387242vPbovG", "223906qcnyvM", "log1p", "12xdPxHN", "process", "36410PdKtQR"];
        return r = function() {
            return e
        }, r()
    }
}
```

As you can probably tell, the code is obfuscated, however fortunately for us the obfuscation used here is https://obfuscator.io/,
a public obfuscation tool that has public deobfuscation tools available, and also is pretty easy to reverse engineer yourself
if you have experience with JavaScript AST libraries, like SWC or Babel.

Unfortunately https://deobfuscate.io/ did not work for me (the browser just froze for a second and produced nothing),
so I decided to make my own deobfuscator using SWC, which can be found in the `src/deobfuscate` directory.

I first noticed what I call "proxy variables" which is a type of transformation obfuscator.io does.
It introduces variables that simply refer to other identifiers (only functions in this case) to
make the deobfuscation process more annoying. Take this example:
```js
function x() {}
var y = x;
y();
```
This code can easily just be:
```js
function x() {}
x();
```
This is what the `proxy_vars` transformer does. It removes these extra variables and modifies all
`CallExpression` nodes to use the real identifier instead.

However, we do also need to be aware of special cases like these:
```js
function x() {}
var y = x;
function doStuff(x) {
    y();
}
```
If we replaced `y()` with `x()` in this case, we'd be pointing to the `x` parameter, which is incorrect.
To see more about how I handled this, take a look at the visitor code yourself in `src/deobfuscate/proxy_vars.rs`.

After dealing with the proxy vars, I reversed the string obfuscation.
Fortunately for me I already knew how their obfuscation works,
but if you don't know, it's pretty simple; an array of strings (the `e` variable in this case) is returned
from the function `r` as a *reference*, meaning the returned array can be modified by callers of `r`.
An IIFE (Immediately Invoked Function Expression) modifies the array, which is where the `parseInt` stuff
comes in: an expression is computed that produces either a number or NaN. If the number doesn't match the
second argument (the constant expression `-170842 + -1 * 92122 + 375877`), then the first element of
the array is removed and pushed to the back of the array. This continues until the expression evaluates to
the correct answer, which then stops the loop. The obfuscated strings (now de-obfuscated) are indexed by
the `x` function, which basically gets the string at *i* where *i* in this case is the given argument
subtracted by `(71 * -137 + 5097 + 4754)`. It's important to note that these expressions change for each
script, and the schematics of the code can also slightly change, since obfuscator.io introduces some randomness.
After we've reversed the strings, we can simply replace all the `CallExpression` nodes with a `StringLiteral` node
by computing the real index (using the offset we mentioned), and simply get the string from the modified array.

After we've reversed the strings, and removed all related code, we now get this:
```js
function(a) {
    return function() {
        return [
            a / Math["log2"](a * Math["LN10"]),
            Object["keys"](globalThis["process"] || {}),
            globalThis["marker"]
        ];
    }();
};
```

This is a lot more readable, and we can now see what the script is really doing; it's returning an array of
three elements, the first being a math expression, the second getting the keys of the `process` object (if it exists),
and the third getting the value of the `globalThis.marker` variable.

After reading this myself, I suspected that the script is not static and was instead randomly generated.
I decided to take another payload from a browser request and decode it, which then showed this code:
```js
function(a) {
    return function() {
        return [
            a - Math["log"](a % Math.E),
            Object["keys"](globalThis["process"] || {}),
            globalThis["marker"]
        ];
    }();
};
```
This confirmed my suspicion that the math expression is random, however the remaining two elements
are static and can be hard-coded.

After applying the computed_member_expr transformation to transform expressions like `Math["log"]` into
`Math.log` to make deigning visitors easier, I began making the math_expr visitor. Unfortunately SWC
does not have a way of evaluating expressions like the one above, so I designed two functions to handle
these math expressions; one function that gets the value of a field (like `Math.PI` -> `3.141592653589793`),
and one that computes a function call (like `Math.max(1, 2)` -> `2`). You can see the code for this and
how I designed these functions in `src/deobfuscate/math_expr.rs`. After we've replaced all these fields
and calls, we're left with a constant expression like `5 * 7 + 1` where we can simply use `expr_simplifier`,
an SWC visitor, that simplifies expressions into a constant value, and then we have the answer to the challenge.

From this point on all I had to do was design the token generation logic which can be found in `src/lib.rs`.

If you run the benchmark using `cargo bench`, you can see that the average execution time is very low
(for me it was 100.66 µs = 0.10066 ms). Running the same script in node and the browser took around 0.11-0.27 ms,
meaning our solution with parsing AST is the same, if not faster, than evaluating the JavaScript code.

## Conclusion
Making bot protection that simply evaluates a math expression and queries the keys of the `process` object
is a very bad idea (especially since math can be platform-dependent, which would lead to incorrect results
server-side). Trying to conceal the token generation request by making its path as an image (`jpeg`) is
completely laughable and does not stop anyone at all.
