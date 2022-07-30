### 实验总结

实现了spawn系统调用，和fork+exec很相似，不过不用拷贝父进程的地址空间。大致的流程是从elf文件中获取相关信息；分配pid和内核栈；初始化实例；设置trap上下文信息。

然后是stride调度算法。包含两步，设置优先级，将Task Manager的fetch方法中默认的先入先出调度方法修改为stride调度方法。不过在实现中会有pass溢出的问题。而rust在通过强制截断的方式来处理溢出。那么需要解决的问题是，在pass累加溢出之后，我们还能找到真正的最小pass的进程。根据定理：

> **在不考虑溢出的情况下** , 在进程优先级全部 >= 2 的情况下，如果严格按照算法执行，那么 PASS_MAX – PASS_MIN <= BigStride / 2。

利用两个pass的差来判定大小，并且需要保证pass的差不会溢出。比如BigStride取值usize::MAX ，那么差可以表示为isize类型，因为isize的范围是[-BigStride/2,BigStride/2-1]，符合要求。

### 问答作业

1. 实际情况是轮到p2执行，p2执行完之后，pass溢出了，成为了4，比255更小。

2. 初始状态，pass都是0，此时轮到优先级为2的进程执行，那么pass_max和pass_min的差值就会扩大到BigStride/2。符合不等式。然后利用归纳法，假设第k次符合不等式，那么选择pass最小值的进程执行之后，pass最小值增加的步长最大为BigStride/2，肯定还是符合不等式的。这里面就不细说明了。

3. ```rust
   use core::cmp::Ordering;
   
   struct Pass(u64);
   
   impl PartialOrd for Pass {
       fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
           let difference = (self.0 - other.0) as i64;
           if difference <= 0 {
               return Some::new(Ordering::Less);
           }else{
               return Some::new(Ordering::Greater);
           }
       }
   }
   
   impl PartialEq for Pass {
       fn eq(&self, other: &Self) -> bool {
           return self.0 == other.0;
       }
   }
   ```

   