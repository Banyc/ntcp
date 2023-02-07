# NTCP

TCP but with a bunch of sockets and a smart scheduler.

## Overview

![arch](img/arch.drawio.png)

- NTCP represents only one stream
- NTCP guarantees that the data is delivered in order, without duplication or loss
- NTCP expects unexpected disconnections of some socket connections

## Scheduler

- say:
  - RTT of a TCP connection inversely represents the quality of the connection
  - $r_i$: the RTT of the $i$-th connection
  - $w_i$: the weight of the $i$-th connection
  - there are $n$ connections
  - $l : \mathbb{R}^n \times \mathbb{R}^n \to \mathbb{R}$: the loss function
  - $l(r, w) = r \cdot w$
  - $w' \in \mathbb{R}^n$: the next weight vector
  - $\alpha \in \mathbb{R}$: the learning rate
- goal: minimize $l$
- the next weight vector $w' \in \mathbb{R}^n$:
  ```math
  w' = \left (\arg\min_{w \in \mathbb{R}^n} l(r, w) \right) \cdot \alpha + w \cdot (1 - \alpha)
  ```
