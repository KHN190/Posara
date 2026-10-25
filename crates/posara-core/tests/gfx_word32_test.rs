#![cfg(feature="gfx")]
use posara_core::word32::gfx;
#[test]
fn coefficients_match_every_original_q16_degree() {
    for d in 0..360 {
        let expected=((d as f64).to_radians().sin()*65536.0).round() as i32;
        assert_eq!(gfx::sin_q16(d),expected,"degree {d}");
    }
}
#[test]
fn narrow_rotation_uses_the_exact_previous_mapping() {
    let dimensions=[(1,1),(2,3),(3,2),(17,24),(4096,4095)];
    for (w,h) in dimensions {
        for d in 0..360 {
            let mut actual=Vec::new();
            gfx::blitr_map_in(w,h,d,-3..4,-3..4,|dx,dy,sx,sy|actual.push((dx,dy,sx,sy)));
            let (s,c)=(gfx::sin_q16(d as usize) as i64,gfx::sin_q16((d as usize+90)%360) as i64);
            let mut expected=Vec::new();
            for dy in -3i64..4 { for dx in -3i64..4 {
                let ox=(dx<<16)+if w%2==0 {32768}else{0};
                let oy=(dy<<16)+if h%2==0 {32768}else{0};
                let sx=(c*ox+s*oy+((w as i64)<<31))>>32;
                let sy=(-s*ox+c*oy+((h as i64)<<31))>>32;
                if sx>=0&&sx<w as i64&&sy>=0&&sy<h as i64 { expected.push((dx as i32,dy as i32,sx as i32,sy as i32)); }
            }}
            assert_eq!(actual,expected,"{w}x{h} angle {d}");
        }
    }
}

#[test]
fn narrow_circle_preserves_full_coordinate_domain_exactly() {
    let limit=1i32<<21;
    for r in [0,1,2,3,46340,46341,1_000_000,limit-1,limit] {
        for d in [0,1,r/2,(r-1).max(0),r] { if d>r {continue;}
            for dy in [-d,d] {
                let x=gfx::circle_span(r,dy) as i64;
                let n=r as i64*r as i64+r as i64-dy as i64*dy as i64;
                assert!(x*x<=n&&(x+1)*(x+1)>n,"r{r} dy{dy} x{x}");
            }
        }
    }
    let mut bits=0x31415926u32;
    for _ in 0..1000000 {
        bits=bits.wrapping_mul(1664525).wrapping_add(1013904223); let r=(bits%(limit as u32+1)) as i32;
        bits=bits.wrapping_mul(1664525).wrapping_add(1013904223); let dy=(bits%(r as u32+1)) as i32;
        let x=gfx::circle_span(r,dy) as i64; let n=r as i64*r as i64+r as i64-dy as i64*dy as i64;
        assert!(x*x<=n&&(x+1)*(x+1)>n);
    }
}
#[test]
fn narrow_triangles_match_wide_at_large_coordinates_and_degeneracy() {
    let mut a=gfx::Framebuffer::new(); let mut b=posara_core::word64::gfx::Framebuffer::new();
    a.configure(8,8,1).unwrap(); b.configure(8,8,1).unwrap();
    let mut bits=1234u32;
    for _ in 0..10000 {
        let points: [i32;6]=core::array::from_fn(|_| {bits=bits.wrapping_mul(1664525).wrapping_add(1013904223); (bits%4194305) as i32-2097152});
        a.cls(0);b.cls(0); let [x,y,u,v,p,q]=points;
        a.tri(x,y,u,v,p,q,65535,true); b.tri(x as i64,y as i64,u as i64,v as i64,p as i64,q as i64,65535,true);
        assert_eq!(a.buf,b.buf,"{points:?}");
    }
    a.cls(0); b.cls(0);
    a.tri(-2097152,-2097152,2097152,2097152,0,0,65535,true);
    b.tri(-2097152,-2097152,2097152,2097152,0,0,65535,true); assert_eq!(a.buf,b.buf);
}

#[test]
fn circle_residual_capacity_covers_the_entire_native_domain() {
    // Exact a,b <= 2^22; positive multiply+add each have error <= 2^18.
    // sqrt <= 2^21+1, half-ULP <= 1/8; floor adds <1, so use 5/4.
    let radius=1u64<<21;
    let input_error=1u64<<19;
    let candidate_error_numerator=5*(2*(radius+1)*4+5);
    let residual_bound=input_error+(candidate_error_numerator+15)/16;
    assert!(residual_bound < 1<<23);
    assert!(residual_bound < i32::MAX as u64);
}

#[test]
fn scaled_short_sprites_and_overflowing_offsets_stay_transparent() {
    let mut small=gfx::Framebuffer::new(); let mut wide=posara_core::word64::gfx::Framebuffer::new();
    small.configure(8,8,1).unwrap(); wide.configure(8,8,1).unwrap();
    small.palette[1]=65535; wide.palette[1]=65535;
    for off in [0,1,i32::MAX as usize,usize::MAX] {
        for pct in [1,100,10000] {
            for (x,y,w,h) in [(0,0,1,1),(-2097151,-2097151,2097152,2097152),(0,0,2097152,2097152)] {
                small.cls(0); wide.cls(0);
                small.blit_4bpp(|i|[0x11u8].get(i).copied().unwrap_or(0),off,x,y,w,h,pct,256);
                wide.blit_4bpp(|i|[0x11u8].get(i).copied().unwrap_or(0),off,x as i64,y as i64,w as i64,h as i64,pct as i64,256);
                assert_eq!(small.buf,wide.buf);
            }
        }
    }
    assert!(small.configure(usize::MAX,2,1).is_err());
}

#[test]
fn rotation_numerator_fits_signed_word_at_every_bounding_corner() {
    let bound=2i64*(2*65536*2898)+2*65536+4096*65536;
    assert!(bound<i32::MAX as i64);
    for degrees in 0..360 {
        for x in [-2898,2895] {for y in [-2898,2895] {
            gfx::blitr_map_in(4096,4096,degrees,x..x+3,y..y+3,|_,_,_,_|{});
        }}
    }
}
