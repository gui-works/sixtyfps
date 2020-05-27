// Using a macro for now.  But there could be others ways to do that
sixtyfps::sixtyfps! {

component TwoRectangle := Rectangle {

    signal clicked;

    Rectangle {
        x: 50;
        y: 50.;
        width: 25;
        height: 25;
        color: red;

        my_area := TouchArea {
            width: 25;
            height: 25;
            clicked => { clicked }
        }

    }
}


component ButtonRectangle := Rectangle {
    signal clicked;
    width: 100;
    height: 75;
    TouchArea {
        width: 100;
        height: 75;
        clicked => { clicked }
    }
}

Hello := Rectangle {

    signal foobar;
    signal plus_clicked;
    signal minus_clicked;

    color: white;

    Text {
        x: 10;
        y: 100;
        text: "Hello World";
        color: black;
    }

    TwoRectangle {
        width: 100;
        height: 100;
        color: blue;
        clicked => { foobar }
    }
    Rectangle {
        x: 100;
        y: 100;
        width: (100);
        height: {100}
        color: green;
        Rectangle {
            x: 50;
            y: 50.;
            width: 25;
            height: 25;
            color: yellow;
        }
    }
    Image {
        x: 200;
        y: 200;
        source: img!"../graphicstest/logo.png";
    }

    ButtonRectangle {
        color: 4289374890;
        x: 50;
        y: 225;
        clicked => { plus_clicked }
        Text {
            x: 50;
            y: 10;
            text: "+";
            color: black;
        }
    }
    counter := Text { x: 100; y: 300; text: "0"; color: black; }
    ButtonRectangle {
        color: 4289374890;
        x: 50;
        y: 350;
        clicked => { minus_clicked }
        Text {
            x: 50;
            y: 10;
            text: "-";
            color: black;
        }
    }

}

}

fn main() {
    Hello::default().run();
}
