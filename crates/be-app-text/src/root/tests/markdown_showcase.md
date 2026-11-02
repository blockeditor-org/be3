# Markdown showcase

Every GitHub Flavored Markdown feature, then the edge cases where they nest. Body text is
**bold**, *italic*, ***both***, ~~struck~~, ~single tilde struck~, and `inline code`.
Underscores work too: __bold__, _italic_, and snake_case_words stay plain.

## Headings

### Third level

#### Fourth level

##### Fifth level

###### Sixth level

Setext heading
==============

Setext second level
-------------------

## Inline

A [link](https://example.com "with a title"), a [reference link][ref], a [collapsed][] one, a
[shortcut] one, an autolink <https://example.com>, a bare www.example.com, a bare
https://example.com/path?q=1, and mail to <someone@example.com>.

Code with a backtick inside: `` a ` b ``. Escapes: \*not italic\*, \# not a heading, \[not a
link\]. Entities: &copy; &amp; &lt;tag&gt; &#8212; &nbsp;done.

A hard break with two trailing spaces  
and one with a backslash\
then a soft break
that joins the line before it.

Inline <kbd>Ctrl</kbd>+<kbd>C</kbd> HTML, and a footnote.[^1]

![An image](https://example.com/image.png "Image title")

[ref]: https://example.com/reference
[collapsed]: https://example.com/collapsed
[shortcut]: https://example.com/shortcut
[^1]: Footnotes are a GitHub extension, not part of the GFM spec.

## Lists

- Dash bullet
* Star bullet, which starts a new list
+ Plus bullet, another new list

1. First
2. Second
3) Parenthesis, a new list

7. Starts at seven
8. Eight

- Tight item
- Tight item

- Loose item

- Loose item, with a second paragraph

  that belongs to it.

- Level one
  - Level two
    - Level three
      - Level four, long enough to wrap onto another line so its continuation hangs under
        its own text rather than under the bullet
  - Back to two
    1. Ordered inside unordered
    2. Second
       - Unordered inside that

- [ ] Task to do
- [x] Task done
  - [ ] Nested task
  - [x] Nested done, with **bold** and a [link](https://example.com)
1. [ ] Ordered task

-
  Item whose content starts on the next line

## Quotes

> A quote with **bold** and `code`,
lazily continued on a line without its marker.
>
> > A nested quote
> > > three deep

> #### A heading in a quote
>
> - A list in a quote
>   ```rust
>   fn in_a_list_in_a_quote() {}
>   ```
> - Second item
>
> | In | A | Quote |
> |----|---|-------|
> | 1  | 2 | 3     |

## Code

```rust
fn main() {
    let greeting = "hello";
    println!("{greeting}, world");
}
```

~~~
A tilde fence with no language,
and a ``` line that does not close it.
~~~

````markdown
```
A fence inside a longer fence.
```
````

    An indented code block,
    four spaces in.

## Tables

| Left | Center | Right | Default |
|:-----|:------:|------:|---------|
| a | **bold** | 1 | `code` |
| longer cell content | b | 22 | [link](https://example.com) |
| escaped \| pipe | | 333 | ~~struck~~ |

Header | Without | Outer pipes
--- | --- | ---
x | y | z

## Breaks and HTML

Above a dash rule.

---

Above a star rule.

***

Above an underscore rule.

___

<details>
<summary>An HTML block</summary>

Markdown inside it, after a blank line.

</details>

<!-- An HTML comment -->

## Edge cases

1. A list item

   > containing a quote
   >
   > ```js
   > console.log("code in a quote in a list item");
   > ```

   and a paragraph after it.

2. A heading in a list item:
   ### Heading
   Text under it.

- A table in a list item:

  | a | b |
  |---|---|
  | 1 | 2 |

- ```
  A fence opened on the marker's line
  ```

> 1. Ordered in a quote
>    - [ ] task in that
>      > quote in the task
>      > > nested once more, with *emphasis spanning
>      > > two lines*

**Bold with *italic* inside** and *italic with **bold** inside* and ~~struck **bold**~~.

[**Bold link text** with `code`](https://example.com) and **[a link in bold](https://example.com)**.

*Unclosed emphasis, **unclosed bold, and a trailing backtick `

A line that is long enough to wrap in any reasonable window, so it shows how wrapped body text sits with its line height and how the markers around **bold text that itself wraps across the end of a line and onto the next** look.

Trailing text with no newline at the end