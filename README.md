# NTCP

TCP but with a bunch of sockets and a smart scheduler.

## Overview

<div style="background-color:Gray">

![arch](img/arch.drawio.png)

</div>

- NTCP represents only one stream
- NTCP guarantees that the data is delivered in order, without duplication or loss
- NTCP expects unexpected disconnections of some socket connections

## Scheduler

- say:
  - RTT of a TCP connection inversely represents the quality of the connection
  - there are $n$ connections
  - $r \in \mathbb{R}^n$: the RTT vector
    - $r_i$: the RTT of the $i$-th connection
  - $w \in \mathbb{R}^n$: the weight vector
    - $\frac{w}{\| w \|_1} = 1$
    - $w_i$: the weight of the $i$-th connection
  - $N(v)$: standardize a vector $v$
  - $l : \mathbb{R}^n \times \mathbb{R}^n \to \mathbb{R}$: the loss function
  - $w' \in \mathbb{R}^n$: the next weight vector
  - $\alpha \in \mathbb{R}$: the learning rate
    - $a \in (10^{-6}, 1)$
- goal: minimize $l$
- the default weight vector $w$:
  ```math
  w = \left( \frac{1}{n}, \dots, \frac{1}{n} \right)
  ```
- the loss function $l$:
  ```math
  l(r, w) = \frac{r}{\| r \|_1} \cdot | w - y |
  ```
  - $y$: an one-hot vector
    - ```math
      y_i =
      \begin{cases}
        1 & i = \arg \min_i r_i \\
        0 & \text{otherwise} \\
      \end{cases}
      ```
- the next weight vector $w' \in \mathbb{R}^n$:
  ```math
  v = w - \alpha \nabla l(r, w) \\
  v_i' =
  \begin{cases}
    0 & v_i < 0 \\
    v_i & v_i \geq 0 \\
  \end{cases} \\
  w' = \frac{v'}{\| v' \|_1} \\
  ```
  - $r$: considered as a constant vector in $\nabla l$
