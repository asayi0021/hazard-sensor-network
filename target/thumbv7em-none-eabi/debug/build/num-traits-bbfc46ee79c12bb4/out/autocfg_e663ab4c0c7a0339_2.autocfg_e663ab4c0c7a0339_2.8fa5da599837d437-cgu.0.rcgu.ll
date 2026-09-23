; ModuleID = 'autocfg_e663ab4c0c7a0339_2.8fa5da599837d437-cgu.0'
source_filename = "autocfg_e663ab4c0c7a0339_2.8fa5da599837d437-cgu.0"
target datalayout = "e-m:e-p:32:32-Fi8-i64:64-v128:64:128-a:0:32-n32-S64"
target triple = "thumbv7em-unknown-none-eabi"

@alloc_f93507f8ba4b5780b14b2c2584609be0 = private unnamed_addr constant [8 x i8] c"\00\00\00\00\00\00\F0?", align 8
@alloc_ef0a1f828f3393ef691f2705e817091c = private unnamed_addr constant [8 x i8] c"\00\00\00\00\00\00\00@", align 8

; autocfg_e663ab4c0c7a0339_2::probe
; Function Attrs: nounwind
define dso_local void @_RNvCsckDjSADL7rn_26autocfg_e663ab4c0c7a0339_25probe() unnamed_addr #0 {
start:
; call <f64>::total_cmp
  %_1 = call i8 @_RNvMNtCs8hqUE7BOfQZ_4core3f64d9total_cmpCsckDjSADL7rn_26autocfg_e663ab4c0c7a0339_2(ptr align 8 @alloc_f93507f8ba4b5780b14b2c2584609be0, ptr align 8 @alloc_ef0a1f828f3393ef691f2705e817091c) #3
  ret void
}

; <f64>::total_cmp
; Function Attrs: inlinehint nounwind
define internal i8 @_RNvMNtCs8hqUE7BOfQZ_4core3f64d9total_cmpCsckDjSADL7rn_26autocfg_e663ab4c0c7a0339_2(ptr align 8 %self, ptr align 8 %other) unnamed_addr #1 {
start:
  %_6 = alloca [8 x i8], align 8
  %_3 = alloca [8 x i8], align 8
  %_5 = load double, ptr %self, align 8
  %_4 = bitcast double %_5 to i64
  store i64 %_4, ptr %_3, align 8
  %_8 = load double, ptr %other, align 8
  %_7 = bitcast double %_8 to i64
  store i64 %_7, ptr %_6, align 8
  %_13 = load i64, ptr %_3, align 8
  %_12 = ashr i64 %_13, 63
  %_10 = lshr i64 %_12, 1
  %0 = load i64, ptr %_3, align 8
  %1 = xor i64 %0, %_10
  store i64 %1, ptr %_3, align 8
  %_18 = load i64, ptr %_6, align 8
  %_17 = ashr i64 %_18, 63
  %_15 = lshr i64 %_17, 1
  %2 = load i64, ptr %_6, align 8
  %3 = xor i64 %2, %_15
  store i64 %3, ptr %_6, align 8
  %4 = load i64, ptr %_3, align 8
  %5 = load i64, ptr %_6, align 8
  %_0 = call i8 @llvm.scmp.i8.i64(i64 %4, i64 %5)
  ret i8 %_0
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare range(i8 -1, 2) i8 @llvm.scmp.i8.i64(i64, i64) #2

attributes #0 = { nounwind "frame-pointer"="all" "target-cpu"="generic" }
attributes #1 = { inlinehint nounwind "frame-pointer"="all" "target-cpu"="generic" }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { nounwind }

!llvm.ident = !{!0}

!0 = !{!"rustc version 1.97.1 (8bab26f4f 2026-07-14)"}
