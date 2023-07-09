use criterion::{criterion_group, criterion_main, Criterion, black_box};

fn criterion_benchmark(c: &mut Criterion) {
    use vercel_anti_bot::generate_token;
    const TEST_DATA: &str = "eyJ0IjoiZXlKaGJHY2lPaUprYVhJaUxDSmxibU1pT2lKQk1qVTJSME5OSW4wLi4yMHA0T3VUcTFDVGRkVXRmLmhxMm4wbkVHOXFwZ2NlbWE2T1Rma1o0d3F2aTJ4SlJqaXd1YVhqTkZIai1ET1JRbDFyUGVaYXFDREdlc19sNXU5NFBTVHpnUHFlN3RNZGZxbUhGemVyRjBpNjJxSzlVV3Z1MDRaaG1iM3R1MjQ1eVJ2aGd1aXdtRmZONEt6VGcuYlRZTXBOZXg1cmhQNnpScFZUVG5NZyIsImMiOiJmdW5jdGlvbihhKXtmdW5jdGlvbiB4KGUscyl7dmFyIHQ9cigpO3JldHVybiB4PWZ1bmN0aW9uKG4saSl7bj1uLSgtODkxNSsyMjczKzMzODcqMik7dmFyIGM9dFtuXTtyZXR1cm4gY30seChlLHMpfShmdW5jdGlvbihlLHMpe2Zvcih2YXIgdD14LG49ZSgpO1tdOyl0cnl7dmFyIGk9cGFyc2VJbnQodCgxNDYpKS8xKigtcGFyc2VJbnQodCgxMzIpKS8yKStwYXJzZUludCh0KDE0MSkpLzMrcGFyc2VJbnQodCgxMzUpKS80KihwYXJzZUludCh0KDEzMykpLzUpKy1wYXJzZUludCh0KDEzOSkpLzYqKHBhcnNlSW50KHQoMTM3KSkvNykrcGFyc2VJbnQodCgxNDcpKS84KihwYXJzZUludCh0KDE0MikpLzkpK3BhcnNlSW50KHQoMTM0KSkvMTArcGFyc2VJbnQodCgxNDApKS8xMSooLXBhcnNlSW50KHQoMTQzKSkvMTIpO2lmKGk9PT1zKWJyZWFrO24ucHVzaChuLnNoaWZ0KCkpfWNhdGNoe24ucHVzaChuLnNoaWZ0KCkpfX0pKHIsLTk4MTA0MystMTMxNDEzKjUrMjI5ODEwMSk7ZnVuY3Rpb24gcigpe3ZhciBlPVtcIm1hcmtlclwiLFwia2V5c1wiLFwiMzEwODk4V21vbnBtXCIsXCI0NDcwNDU2SVFmZVZhXCIsXCI2S1BveGN4XCIsXCI3NzM5NWVUWHJTWFwiLFwiNTE4MjczMFZjcXRyZlwiLFwiMjI4eGVweWxhXCIsXCJsb2cxcFwiLFwiODQ3bXJJbmFHXCIsXCJwcm9jZXNzXCIsXCI2NTM1OG1KTGJVRlwiLFwiNDQzM1ZMS3JzclwiLFwiMjkxMzMxMlNQRlNpTVwiLFwiOVl0RkRXUlwiLFwiNTg4dUJIUU5MXCJdO3JldHVybiByPWZ1bmN0aW9uKCl7cmV0dXJuIGV9LHIoKX1yZXR1cm4gZnVuY3Rpb24oKXt2YXIgZT14O3JldHVyblthK01hdGhbZSgxMzYpXShhL01hdGguUEkpLE9iamVjdFtlKDE0NSldKGdsb2JhbFRoaXNbZSgxMzgpXXx8e30pLGdsb2JhbFRoaXNbZSgxNDQpXV19KCl9IiwiYSI6MC42NzM3ODM4NzE5MjA3MTEyfQ==";

    c.bench_function("generate_token", |b| b.iter(|| generate_token(black_box(TEST_DATA))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
