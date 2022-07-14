//! Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.


// use std::process;

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::config::{CLOCK_FREQ, BIG_STRIDE};
use crate::mm::{VirtPageNum, PhysPageNum, MapPermission, VirtAddr};
use crate::sync::UPSafeCell;
use crate::syscall::TaskInfo;
use crate::timer::get_time;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;


/// Processor management structure
pub struct Processor {
    /// The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,
    /// The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
//        let l = self.ready_queue.len();
//        let mut index = 0;
//        let mut min_stride = self.ready_queue[index].inner_exclusive_access().stride;;
//        for i in 1..l{
//            let t = self.ready_queue[i].inner_exclusive_access().stride;
//            if t <= min_stride{
//                min_stride = t;
//                index = i;
//            }
//        }
//        let p = self.ready_queue[index].inner_exclusive_access().priority as usize;
//        self.ready_queue[index].inner_exclusive_access().stride += BIG_STRIDE / p;
//        self.ready_queue.remove(index)
        &mut self.idle_task_cx as *mut _
//        &mut return_least_stride_contex()
    }
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(|task| Arc::clone(task))
    }
}

lazy_static! {
    /// PROCESSOR instance through lazy_static!
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

/// The main part of process execution and scheduling
///
/// Loop fetch_task to get the process that needs to run,
/// and switch the process through __switch
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;

            let p = task_inner.priority;
            let add_res = task_inner.stride.wrapping_add(BIG_STRIDE / p);
            task_inner.stride = add_res;

            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get token of the address space of current task
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.inner_exclusive_access().get_user_token();
    token
}

/// Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

/// Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

pub fn translate(vpn: VirtPageNum)-> PhysPageNum{
    let curPCB = current_task().unwrap();
    let mem_set = &curPCB.inner_exclusive_access().memory_set;
    let ppn = mem_set.translate(vpn).unwrap().ppn();
    return ppn;

}

pub fn update_syscall_times(syscall_id: usize){
    let curPCB = current_task().unwrap();
    let mut inner = curPCB.inner_exclusive_access();
    inner.syscall_times[syscall_id] += 1;

}
pub fn set_task_info(taskinfo: *mut TaskInfo) {
    let curPCB = current_task().unwrap();
    let mut inner = curPCB.inner_exclusive_access();
    let t = (get_time() - inner.task_begin_time)*1000/CLOCK_FREQ;
    unsafe {
        *taskinfo = TaskInfo{
            // status: inner.tasks[inner.current_task].task_status,
            status: TaskStatus::Running,
            syscall_times: inner.syscall_times,
            time: t,
        };
    }
}

pub fn set_priority(_prio: u8) {
    let curPCB = current_task().unwrap();
    let mut inner = curPCB.inner_exclusive_access();
    inner.priority = _prio;
}

pub fn contains_key(vpn: &VirtPageNum)-> bool {
    let curPCB = current_task().unwrap();
    let mut inner = curPCB.inner_exclusive_access();
    inner.memory_set.contains_key(vpn)
}

pub fn mmap(start_va: VirtAddr, end_va: VirtAddr, permission: MapPermission) {
    let curPCB = current_task().unwrap();
    let mut inner = curPCB.inner_exclusive_access();
    inner.memory_set.insert_framed_area(start_va, end_va, permission);
}

pub fn m_numap(start_vpn: VirtPageNum, end_vpn: VirtPageNum)-> isize {
    let curPCB = current_task().unwrap();
    let mut inner = curPCB.inner_exclusive_access();
    inner.memory_set.unmap(start_vpn, end_vpn)
}
