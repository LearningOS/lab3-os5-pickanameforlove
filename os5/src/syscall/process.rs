//! Process management syscalls

use crate::config::{MAX_SYSCALL_NUM, PAGE_SIZE};
use crate::loader::get_app_data_by_name;
use crate::mm::{translated_refmut, translated_str, VirtAddr, VirtPageNum, MapPermission};
use crate::task::{
    add_task, current_task, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next, translate, TaskStatus, set_task_info, set_priority, contains_key, mmap, m_numap
};
use crate::timer::get_time_us;
use alloc::sync::Arc;
use riscv::addr::page;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    debug!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

/// Syscall Fork which returns 0 for child process and child_pid for parent process
pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

/// Syscall Exec which accepts the elf path
pub fn sys_exec(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();
    // find a child process

    // ---- access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB lock exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after removing from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child TCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB lock automatically
}

// YOUR JOB: 引入虚地址后重写 sys_get_time
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    let _us = get_time_us();
    let vaddr = _ts as usize;
    let vaddr_obj = VirtAddr(vaddr);
    let page_off = vaddr_obj.page_offset();

    let vpn = vaddr_obj.floor();

    let ppn = translate(vpn);

    let paddr = ppn.0 << 12 | page_off;
    let ts = paddr as *mut TimeVal;

    unsafe {
        *ts = TimeVal {
            sec: _us / 1_000_000,
            usec: _us % 1_000_000,
        };
    }
    0
}

// YOUR JOB: 引入虚地址后重写 sys_task_info
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    let vaddr = ti as usize;
    let vaddr_obj = VirtAddr(vaddr);
    let page_off = vaddr_obj.page_offset();

    let vpn = vaddr_obj.floor();

    let ppn = translate(vpn);

    let paddr : usize = ppn.0 << 12 | page_off;
    let ts  = paddr as *mut TaskInfo;
    set_task_info(ts);
    0
}

// YOUR JOB: 实现sys_set_priority，为任务添加优先级
pub fn sys_set_priority(_prio: isize) -> isize {
    if _prio < 2 {
        return -1;
    }
    set_priority(_prio);
    _prio
}

// YOUR JOB: 扩展内核以实现 sys_mmap 和 sys_munmap
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    if _port & !0x7 != 0 || _port & 0x7 == 0 || _len <= 0 || _start % PAGE_SIZE  !=0{
        return -1;
    }

    let _end = _start + _len;

    let start_vpn = VirtAddr(_start).floor();
    let end_vpn= VirtAddr(_end).ceil();

    let mut res = false;
    for i in start_vpn.0..end_vpn.0{
        res = res | contains_key(&VirtPageNum(i));
    }
    if res{
        return -1;
    }

    let p = (_port << 1) | 16;
    let permission = MapPermission::from_bits(p as u8).unwrap();
    mmap(VirtAddr(_start) , VirtAddr(_end), permission);

    0
}

pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    if _len <= 0 || _start % PAGE_SIZE !=0{
        return -1;
    }
    let _end = _start + _len;

    let start_vpn: VirtPageNum = VirtAddr(_start).floor();
    let end_vpn: VirtPageNum = VirtAddr(_end).ceil();
    let mut res = true;
    for i in start_vpn.0..end_vpn.0{
        res = res & contains_key(&VirtPageNum(i));
    }
    if !res{
        return -1;
    }
    m_numap(start_vpn, end_vpn)
}

//
// YOUR JOB: 实现 sys_spawn 系统调用
// ALERT: 注意在实现 SPAWN 时不需要复制父进程地址空间，SPAWN != FORK + EXEC
pub fn sys_spawn(_path: *const u8) -> isize {
    let current_task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, _path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let new_task = current_task.spawn(data);
        let new_pid = new_task.pid.0;
        // modify trap context of new_task, because it returns immediately after switching
//        let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
        // we do not have to move to next instruction since we have done it before
        // for child process, fork returns 0
//        trap_cx.x[10] = 0;
        // add new task to scheduler
        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}
