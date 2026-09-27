import SwiftUI

/// Patina's mark: a P whose counter is a play triangle (assets/mark.svg).
/// Fill it with `.fill(Mark.verdigris, style: FillStyle(eoFill: true))`
/// so the triangle stays cut out.
struct Mark: Shape {
    static let verdigris = Color(hex: "#7fb3a0")!
    /// Width over height.
    static let aspect: CGFloat = 360.0 / 500.0

    func path(in rect: CGRect) -> Path {
        // Drawn in the SVG's 360×500 box, then scaled into `rect`.
        var p = Path()
        p.move(to: CGPoint(x: 0, y: 0))
        p.addLine(to: CGPoint(x: 200, y: 0))
        p.addArc(center: CGPoint(x: 200, y: 160), radius: 160, startAngle: .degrees(-90), endAngle: .degrees(90), clockwise: false)
        p.addLine(to: CGPoint(x: 68, y: 320))
        p.addLine(to: CGPoint(x: 68, y: 500))
        p.addLine(to: CGPoint(x: 0, y: 500))
        p.closeSubpath()
        var play = Path()
        play.move(to: CGPoint(x: 138, y: 82))
        play.addLine(to: CGPoint(x: 286, y: 160))
        play.addLine(to: CGPoint(x: 138, y: 238))
        play.closeSubpath()
        p.addPath(play)
        let scale = min(rect.width / 360, rect.height / 500)
        let dx = rect.minX + (rect.width - 360 * scale) / 2
        let dy = rect.minY + (rect.height - 500 * scale) / 2
        return p.applying(CGAffineTransform(a: scale, b: 0, c: 0, d: scale, tx: dx, ty: dy))
    }
}

/// The mark in verdigris at a height.
struct Logo: View {
    var height: CGFloat = 40

    var body: some View {
        Mark()
            .fill(Mark.verdigris, style: FillStyle(eoFill: true))
            .frame(width: height * Mark.aspect, height: height)
            .accessibilityLabel("Patina")
    }
}
